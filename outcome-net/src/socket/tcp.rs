use std::collections::{HashMap, VecDeque};
use std::convert::{TryFrom, TryInto};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread::{sleep, yield_now, JoinHandle};
use std::time::Duration;

use byteorder::{ByteOrder, LittleEndian};

use crate::msg::{Message, MessageType};
use crate::socket::{Encoding, SocketAddress, SocketConfig, SocketEvent, SocketEventType};
use crate::{
    error::{Error, Result},
    sig::Signal,
};
use fnv::FnvHashMap;

/// Custom Tcp socket.
pub struct TcpSocket {
    pub config: SocketConfig,
    listener_addr: Option<SocketAddress>,
    connections: Vec<(SocketAddress)>,
    poll_handle: Option<JoinHandle<()>>,
    in_receiver: Receiver<(SocketAddress, SocketEvent)>,
    out_sender: Sender<(SocketAddress, SocketEvent)>,
    event_backlog: VecDeque<(SocketAddress, SocketEvent)>,
}

impl TcpSocket {
    pub fn new(addr: Option<SocketAddress>) -> Result<Self> {
        let config = SocketConfig {
            ..Default::default()
        };
        Self::new_with_config(addr, config)
    }

    pub fn new_with_config(addr: Option<SocketAddress>, config: SocketConfig) -> Result<Self> {
        let listener = if let Some(_addr) = addr {
            let socket_addr: SocketAddr = _addr.clone().try_into()?;
            let mut listener = TcpListener::bind(socket_addr)?;
            listener
                .set_nonblocking(true)
                .expect("Cannot set non-blocking");

            let _addr = SocketAddress::Net(listener.local_addr()?);
            trace!("binding listener to: {:?}", _addr);
            Some(listener)
        } else {
            None
        };
        let listener_addr = listener
            .as_ref()
            .map(|l| SocketAddress::Net(l.local_addr().unwrap()))
            .clone();

        let (out_sender, out_receiver) = channel();
        let (in_sender, in_receiver) = channel();

        // let addr = listener.local_addr().unwrap();

        let mut handler = ConnectionHandler {
            listener,
            connections: Default::default(),
            in_sender,
            out_receiver,
            out_sender: out_sender.clone(),
            heartbeat_interval: config.heartbeat_interval,
            time_since_heartbeat: Default::default(),
        };

        // Starts the poll mechanism to receive and send messages
        let poll_handle = std::thread::spawn(move || handler.start_polling());

        Ok(Self {
            config,
            listener_addr,
            connections: Vec::new(),
            poll_handle: Some(poll_handle),
            in_receiver,
            out_sender,
            event_backlog: VecDeque::new(),
        })
    }

    // pub fn bind(&mut self, addr: SocketAddress) -> Result<()> {
    //     self.
    // }

    pub fn connect(&mut self, addr: SocketAddress) -> Result<()> {
        self.connections.push(addr.clone());
        self.out_sender
            .send((addr, SocketEvent::new(SocketEventType::Connect)))
            .unwrap();
        Ok(())
    }

    pub fn disconnect(&mut self, addr: Option<SocketAddress>) -> Result<()> {
        let addr: SocketAddress = match addr {
            Some(a) => a,
            None => match self.connections.first() {
                Some(a) => a.clone(),
                None => {
                    return Err(Error::Other(
                        "no connections left to disconnect".to_string(),
                    ))
                }
            },
        };
        if let Some(a) = self.connections.iter().position(|a| a == &addr) {
            self.connections.remove(a);
            self.out_sender
                .send((addr, SocketEvent::new(SocketEventType::Disconnect)));
        }

        //std::thread::sleep(Duration::from_millis(100));
        //// stop the polling thread
        //if let Some(handle) = self.poll_handle.take() {
        //handle.join().expect("failed to join thread");
        //}

        Ok(())
    }

    pub fn encoding(&self) -> &Encoding {
        &self.config.encoding
    }

    pub fn peer_addr(&self) -> Result<SocketAddress> {
        self.connections
            .first()
            .map(|a| a.clone())
            .ok_or(Error::SocketNotConnected)
    }

    pub fn listener_addr(&self) -> Result<SocketAddress> {
        self.listener_addr
            .clone()
            .ok_or(Error::SocketNotBoundToAddress)
    }

    /// Waits for the next socket event, blocking until one is available.
    pub fn recv(&mut self) -> Result<(SocketAddress, SocketEvent)> {
        let (addr, event) = self
            .in_receiver
            .recv()
            .map_err(|e| Error::Other(e.to_string()))?;
        self.handle_internally(&addr, &event);
        match event.type_ {
            SocketEventType::Disconnect => return Err(Error::HostUnreachable),
            _ => (),
        }
        Ok((addr, event))
    }

    pub fn recv_msg_from(&mut self, addr: SocketAddr) -> Result<Message> {
        unimplemented!()
    }

    pub fn recv_msg(&mut self) -> Result<(SocketAddress, Message)> {
        loop {
            let (addr, event) = self.recv()?;
            let msg = match event.type_ {
                SocketEventType::Bytes => {
                    return Ok((addr, Message::from_bytes(event.bytes, self.encoding())?))
                }
                _ => {
                    self.event_backlog.push_back((addr, event));
                    continue;
                }
            };
        }
    }

    pub fn recv_sig(&mut self) -> Result<(SocketAddress, Signal)> {
        loop {
            let (addr, event) = self.recv()?;
            let msg = match event.type_ {
                SocketEventType::Bytes => {
                    return Ok((addr, Signal::from_bytes(&event.bytes, self.encoding())?))
                }
                _ => {
                    self.event_backlog.push_back((addr, event));
                    continue;
                }
            };
        }
    }

    /// Tries receiving next socket event, returning immediately if there are
    /// none available.
    pub fn try_recv(&mut self) -> Result<(SocketAddress, SocketEvent)> {
        let (addr, event) = match self.in_receiver.try_recv() {
            Ok(event) => event,
            Err(_) => return Err(crate::Error::WouldBlock),
        };
        self.handle_internally(&addr, &event);
        Ok((addr, event))
    }

    pub fn try_recv_msg(&mut self) -> Result<(SocketAddress, Message)> {
        loop {
            match self.try_recv() {
                Ok((addr, socket_event)) => match socket_event.type_ {
                    SocketEventType::Bytes => {
                        return Ok((
                            addr,
                            Message::from_bytes(socket_event.bytes, self.encoding())?,
                        ))
                    }
                    _ => {
                        self.event_backlog.push_back((addr, socket_event));
                        continue;
                    }
                },
                Err(_) => return Err(crate::Error::WouldBlock),
            };
        }
    }

    pub fn try_recv_sig(&mut self) -> Result<(SocketAddress, crate::sig::Signal)> {
        loop {
            // println!("try recv loop");
            match self.try_recv() {
                Ok((addr, event)) => match event.type_ {
                    SocketEventType::Bytes => {
                        return Ok((
                            addr,
                            crate::sig::Signal::from_bytes(&event.bytes, &Encoding::Bincode)?,
                        ))
                    }
                    _ => {
                        trace!("expected bytes, got: {:?}", event);
                        self.event_backlog.push_back((addr, event));
                        continue;
                    }
                },
                Err(_) => return Err(crate::Error::WouldBlock),
            };
        }
        //let (addr, event) = match self.try_recv()? {
        //=> return Ok((addr, Message::from_bytes(bytes)?)),
        //_ => {
        //self.event_backlog.push_back((addr, event));
        //continue;
        //}
        //};
        //self.handle_internally(&addr, &event);
        //Ok((addr, event))
    }

    pub fn send_bytes(&self, bytes: Vec<u8>, addr: Option<SocketAddress>) -> Result<()> {
        self.out_sender
            .send((
                addr.unwrap_or(
                    self.connections
                        .first()
                        .ok_or(Error::SocketNotConnected)?
                        .clone(),
                ),
                SocketEvent::new_bytes(bytes),
            ))
            .map_err(|e| Error::Other(e.to_string()));
        Ok(())
    }

    pub fn send_event(&self, event: SocketEvent, addr: Option<SocketAddress>) -> Result<()> {
        self.out_sender
            .send((
                addr.unwrap_or(
                    self.connections
                        .first()
                        .ok_or(Error::SocketNotConnected)?
                        .clone(),
                ),
                event,
            ))
            .map_err(|e| Error::Other(e.to_string()));
        Ok(())
    }

    //pub fn send_msg(&mut self, msg: Message) -> Result<()> {
    //let packet =
    //laminar::Packet::unreliable_sequenced(self.endpoint_addr.unwrap(), bytes, Some(1));
    //self.sender.send(packet).unwrap();
    //Ok(())
    //}

    /// Some events necessitate changes to socket's state.
    fn handle_internally(&mut self, addr: &SocketAddress, event: &SocketEvent) {
        match &event.type_ {
            SocketEventType::Connect => self.connections.push(addr.clone()),
            SocketEventType::Disconnect => {
                if let Some(idx) = self.connections.iter().position(|a| a == addr) {
                    self.connections.remove(idx);
                }
            }
            _ => (),
        }
    }
}

struct ConnectionHandler {
    listener: Option<TcpListener>,
    connections: FnvHashMap<SocketAddress, (TcpStream, Vec<u8>)>,
    in_sender: Sender<(SocketAddress, SocketEvent)>,
    out_receiver: Receiver<(SocketAddress, SocketEvent)>,
    out_sender: Sender<(SocketAddress, SocketEvent)>,
    heartbeat_interval: Option<Duration>,
    time_since_heartbeat: Duration,
}

impl ConnectionHandler {
    pub fn start_polling(&mut self) {
        self.start_polling_with_duration(Some(Duration::from_millis(1)))
    }

    pub fn start_polling_with_duration(&mut self, sleep_duration: Option<Duration>) {
        let mut last_time = std::time::Instant::now();
        loop {
            let now = std::time::Instant::now();
            let delta_time = now - last_time;
            last_time = now;
            //self.manual_poll(delta_time);
            match self.manual_poll(delta_time) {
                Ok(_) => (),
                Err(e) => error!("manual_poll error: {:?}", e),
            }
            match sleep_duration {
                None => yield_now(),
                Some(duration) => sleep(duration),
            };
        }
    }

    /// Performs all the necessary operations to maintain a socket.
    ///
    /// Delta time argument represents duration since last manual poll call.
    pub fn manual_poll(&mut self, delta_time: Duration) -> Result<()> {
        // send heartbeats
        if let Some(heartbeat) = self.heartbeat_interval {
            self.time_since_heartbeat += delta_time;
            if self.time_since_heartbeat > heartbeat {
                self.time_since_heartbeat = Duration::from_millis(0);
                for (addr, _) in &self.connections {
                    self.out_sender
                        .send((addr.clone(), SocketEvent::new(SocketEventType::Heartbeat)));
                }
            }
        }

        // read incoming events
        for (addr, (stream, buffer)) in &mut self.connections {
            // read from stream into the connection buffer
            // TODO perhaps don't read more if the buffer is really backed up
            loop {
                let mut buf = [0; 8192];
                let read_count = match stream.read(&mut buf) {
                    Ok(count) => count,
                    // TODO handle errors
                    _ => 0,
                };
                if read_count > 0 {
                    buffer.extend(&buf[..read_count]);
                } else {
                    break;
                }
            }

            if buffer.len() > 4 {
                let len = LittleEndian::read_u32(&buffer[0..=3]);
                if buffer.len() >= len as usize + 4 {
                    if let Ok(event) =
                        bincode::deserialize::<SocketEvent>(&buffer[4..len as usize + 4])
                    {
                        self.in_sender
                            .send((addr.clone(), event))
                            .map_err(|e| Error::Other(e.to_string()))?;
                        buffer.drain(..len as usize + 4);
                    } else {
                        continue;
                    }
                }
            }
        }

        // grab all the waiting events and send them over
        while let Ok((address, event)) = self.out_receiver.try_recv() {
            match &event.type_ {
                SocketEventType::Connect => {
                    let socket_addr: SocketAddr = address.clone().try_into()?;
                    let stream = match TcpStream::connect(socket_addr) {
                        Ok(s) => s,
                        Err(e) => {
                            // TODO better retry behavior
                            self.out_sender
                                .send((address, event))
                                .map_err(|e| Error::Other(e.to_string()))?;
                            continue;
                        }
                    };
                    println!("my real address: {:?}", stream.local_addr()?);

                    stream.set_nonblocking(true);
                    stream.set_nodelay(true);

                    self.connections
                        .insert(address.clone(), (stream, Vec::new()));
                }
                // SocketEvent::Disconnect => {
                //     self.connections.remove(&address);
                //     // stream.shutdown(std::net::Shutdown::Both);
                //     continue;
                // }
                _ => (),
            }

            // encode the event
            let bytes = bincode::serialize(&event)?;

            // encode the length
            let mut len_buf = [0; 4];
            LittleEndian::write_u32(&mut len_buf, bytes.len() as u32);

            if let Some((stream, _)) = self.connections.get_mut(&address) {
                // write to stream
                if let Ok(()) = stream.write_all(&len_buf) {
                    stream.write_all(&bytes);
                }
            } else {
                // TODO better retry behavior
                self.out_sender.send((address.clone(), event));
                continue;
            }

            match &event.type_ {
                SocketEventType::Disconnect => {
                    // TODO explicit shutdown?
                    // if let Some((stream, _)) = self.connections.get_mut(&address) {
                    //     if let Err(e) = stream.shutdown(std::net::Shutdown::Both) {
                    //         warn!("failed shutting down stream: {}", e);
                    //     }
                    // }
                    self.connections.remove(&address);
                    return Ok(());
                }
                _ => (),
            }
        }

        // accept new connections, if there are any
        if let Some(listener) = &self.listener {
            for stream in listener.incoming() {
                match stream {
                    Ok(s) => {
                        // info!("incoming stream");
                        //s.set_read_timeout(Some(Duration::from_millis(1)))
                        //.expect("set_read_timeout call failed");
                        println!("accepting new connection: {:?}", s.peer_addr()?);
                        s.set_nonblocking(true);
                        s.set_nodelay(true);
                        self.connections
                            .insert(SocketAddress::Net(s.peer_addr()?), (s, Vec::new()));
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(e) => return Err(Error::Other(format!("{:?}", e))),
                }
            }
        }

        Ok(())
    }

    pub fn connections(&self) -> Vec<SocketAddress> {
        self.connections.iter().map(|(a, _)| a.clone()).collect()
    }
}
