use std::collections::{HashMap, VecDeque};
use std::convert::TryFrom;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread::{sleep, yield_now, JoinHandle};
use std::time::Duration;

use byteorder::{ByteOrder, LittleEndian};

use crate::msg::{unpack, Message, MessageType};
use crate::socket::{Encoding, SocketConfig, SocketEvent};
use crate::{
    error::{Error, Result},
    sig::Signal,
};

/// Custom Tcp socket.
pub struct TcpSocket {
    pub config: SocketConfig,
    address: Option<SocketAddr>,

    connections: Vec<SocketAddr>,
    poll_handle: Option<JoinHandle<()>>,
    in_receiver: Receiver<(SocketAddr, SocketEvent)>,
    out_sender: Sender<(SocketAddr, SocketEvent)>,
    event_backlog: VecDeque<(SocketAddr, SocketEvent)>,
}

impl TcpSocket {
    pub fn bind(addr: &str) -> Result<Self> {
        let config = SocketConfig {
            ..Default::default()
        };
        Self::bind_with_config(addr, config)
    }

    pub fn bind_with_config(addr: &str, config: SocketConfig) -> Result<Self> {
        let input_addr: SocketAddr = addr.parse().unwrap();
        let mut listener = TcpListener::bind(input_addr)?;
        listener
            .set_nonblocking(true)
            .expect("Cannot set non-blocking");

        //let handler = ConnectionHandler::new("")?;

        //let receiver = socket.get_event_receiver();
        //let sender = socket.get_packet_sender();
        //let receiver = channel::unbounder();
        let (out_sender, out_receiver) = channel();
        let (in_sender, in_receiver) = channel();

        let addr = listener.local_addr().unwrap();

        let mut handler = ConnectionHandler {
            listener,
            //stream: None,
            connections: Vec::new(),
            in_sender,
            //in_buffer: VecDeque::new(),
            out_receiver,
            out_sender: out_sender.clone(),
            //out_buffer: VecDeque::new(),
            //in_receiver: in_receiver.clone(),
            //out_sender: out_sender.clone(),
            heartbeat_interval: config.heartbeat_interval,
            time_since_heartbeat: Default::default(),
        };

        // Starts the poll mechanism to receive and send messages
        let poll_handle = std::thread::spawn(move || handler.start_polling());

        Ok(Self {
            config,
            address: Some(addr),
            connections: Vec::new(),
            poll_handle: Some(poll_handle),
            in_receiver,
            out_sender,
            event_backlog: VecDeque::new(),
        })
    }

    pub fn encoding(&self) -> &Encoding {
        &self.config.encoding
    }

    pub fn connect(&mut self, addr: &str) -> Result<()> {
        let addr: SocketAddr = addr.parse().unwrap();
        //self.address = Some(addr.clone());
        self.connections.push(addr.clone());
        self.out_sender.send((addr, SocketEvent::Connect)).unwrap();
        Ok(())
    }

    pub fn disconnect(&mut self, addr: Option<SocketAddr>) -> Result<()> {
        let addr: SocketAddr = match addr {
            Some(a) => a,
            None => match self.connections.first() {
                Some(a) => *a,
                None => {
                    return Err(Error::Other(
                        "no connections left to disconnect".to_string(),
                    ))
                }
            },
        };
        self.connections
            .remove(self.connections.iter().position(|a| a == &addr).unwrap());
        self.out_sender.send((addr, SocketEvent::Disconnect));

        //std::thread::sleep(Duration::from_millis(100));
        //// stop the polling thread
        //if let Some(handle) = self.poll_handle.take() {
        //handle.join().expect("failed to join thread");
        //}

        Ok(())
    }

    pub fn last_endpoint(&self) -> Result<SocketAddr> {
        self.address
            .ok_or(Error::Other("socket not bound to address".to_string()))
        //self.endpoint_addr
        //.map(|s| s.to_string())
        //.ok_or(Error::Other("".to_string()))
    }

    /// Waits for the next socket event, blocking until one is available.
    pub fn recv(&mut self) -> Result<(SocketAddr, SocketEvent)> {
        let (addr, event) = self
            .in_receiver
            .recv()
            .map_err(|e| Error::Other(e.to_string()))?;
        self.handle_internally(&addr, &event);
        match event {
            SocketEvent::Disconnect => return Err(Error::HostUnreachable),
            _ => (),
        }
        Ok((addr, event))
    }

    pub fn recv_msg_from(&mut self, addr: SocketAddr) -> Result<Message> {
        unimplemented!()
    }

    pub fn recv_msg(&mut self) -> Result<(SocketAddr, Message)> {
        loop {
            let (addr, event) = self.recv()?;
            let msg = match event {
                SocketEvent::Bytes(bytes) => return Ok((addr, Message::from_bytes(bytes)?)),
                // SocketEvent::Message(msg) => return Ok((addr, msg)),
                _ => {
                    self.event_backlog.push_back((addr, event));
                    continue;
                }
            };
        }
    }

    pub fn recv_sig(&mut self) -> Result<(SocketAddr, Signal)> {
        loop {
            let (addr, event) = self.recv()?;
            let msg = match event {
                SocketEvent::Bytes(bytes) => {
                    return Ok((addr, Signal::from_bytes(&bytes, self.encoding())?))
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
    pub fn try_recv(&mut self) -> Result<(SocketAddr, SocketEvent)> {
        let (addr, event) = match self.in_receiver.try_recv() {
            Ok(event) => event,
            Err(_) => return Err(crate::Error::WouldBlock),
        };
        self.handle_internally(&addr, &event);
        Ok((addr, event))
    }

    pub fn try_recv_msg(&mut self) -> Result<(SocketAddr, Message)> {
        loop {
            match self.try_recv() {
                Ok((addr, socket_event)) => match socket_event {
                    SocketEvent::Bytes(bytes) => return Ok((addr, Message::from_bytes(bytes)?)),
                    // SocketEvent::Bytes(bytes) => {
                    //     return Ok((addr, unpack(&bytes, self.encoding())?))
                    // }
                    _ => {
                        self.event_backlog.push_back((addr, socket_event));
                        continue;
                    }
                },
                Err(_) => return Err(crate::Error::WouldBlock),
            };
        }
    }

    pub fn try_recv_sig(&mut self) -> Result<(SocketAddr, crate::sig::Signal)> {
        loop {
            // println!("try recv loop");
            match self.try_recv() {
                Ok((addr, event)) => match event {
                    SocketEvent::Bytes(bytes) => {
                        return Ok((
                            addr,
                            crate::sig::Signal::from_bytes(&bytes, &Encoding::Bincode)?,
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

    pub fn send(&self, bytes: Vec<u8>, addr: Option<SocketAddr>) -> Result<()> {
        self.out_sender
            .send((
                addr.unwrap_or(*self.connections.first().unwrap()),
                SocketEvent::Bytes(bytes),
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
    fn handle_internally(&mut self, addr: &SocketAddr, event: &SocketEvent) {
        match &event {
            SocketEvent::Connect => self.connections.push(addr.clone()),
            SocketEvent::Disconnect => {
                if let Some(idx) = self.connections.iter().position(|a| a == addr) {
                    self.connections.remove(idx);
                }
            }
            _ => (),
        }
    }
}

struct ConnectionHandler {
    listener: TcpListener,

    connections: Vec<(SocketAddr, (TcpStream, Vec<u8>))>,
    //receive_buffers: Vec<(SocketAddr, Vec<u8>)>,
    //in_receiver: Receiver<SocketEvent>,
    //out_sender: Sender<Vec<u8>>,
    in_sender: Sender<(SocketAddr, SocketEvent)>,
    //in_buffer: VecDeque<(SocketAddr, SocketEvent)>,
    out_receiver: Receiver<(SocketAddr, SocketEvent)>,
    out_sender: Sender<(SocketAddr, SocketEvent)>,
    //out_buffer: VecDeque<(SocketAddr, SocketEvent)>,
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
                Err(e) => println!("manual_poll error: {:?}", e),
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
                    self.out_sender.send((*addr, SocketEvent::Heartbeat));
                }
            }
        }

        // read incoming events
        for (addr, (stream, buffer)) in &mut self.connections {
            let mut data: Vec<u8> = Vec::new();
            loop {
                let mut buf = [0; 128000];
                // let read_count = match stream.read(&mut buf) {
                let read_count = match stream.read(&mut buf) {
                    Ok(count) => count,
                    _ => 0,
                    // Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // continue;
                    // }
                    // Err(e) => {
                    // warn!("manual poll: io error: {}", e);
                    // continue;
                    // }
                };

                //println!("read_count: {}", read_count);
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
                        // let event = Self::match_event(&buffer[4..len as usize + 4], addr)?;
                        self.in_sender
                            .send((*addr, event))
                            .map_err(|e| Error::Other(e.to_string()))?;
                        //println!("{:?}", buffer);
                        // println!("buffer length: {}, starting drain...", len);
                        buffer.drain(..len as usize + 4);
                        // println!("finished drain");
                    } else {
                        continue;
                    }
                }
            }
        }

        // grab all the waiting events and send them over
        while let Ok((address, event)) = self.out_receiver.try_recv() {
            //warn!("{:?}", event);
            //println!("manual poll: sending");
            //println!("sending: {:?}", event);
            match &event {
                SocketEvent::Connect => {
                    let stream = match TcpStream::connect(address) {
                        Ok(s) => s,
                        Err(e) => {
                            //warn!("{:?}", e);
                            self.out_sender
                                .send((address, event))
                                .map_err(|e| Error::Other(e.to_string()))?;
                            //self.out_buffer.push_back((address, event));
                            //return Ok(());
                            continue;
                        }
                    };
                    //stream
                    //.set_read_timeout(Some(Duration::from_millis(1)))
                    //.expect("set_read_timeout call failed");
                    stream.set_nonblocking(true);
                    stream.set_nodelay(true);
                    self.connections.push((address, (stream, Vec::new())));
                    //info!("added connection");
                }
                //SocketEvent::Disconnect(addr) => {
                //let (_, stream) = self.connections.iter().find(|(a, _)| a == addr).unwrap();
                //stream.shutdown(std::net::Shutdown::Both);
                //continue;
                //}
                _ => (),
            }

            // let bytes = match event.clone() {
            //     SocketEvent::Connect => Message {
            //         type_: MessageType::Connect,
            //         payload: Vec::new(),
            //     }
            //     .to_bytes()?,
            //     SocketEvent::Disconnect => Message {
            //         type_: MessageType::Disconnect,
            //         payload: Vec::new(),
            //     }
            //     .to_bytes()?,
            //     SocketEvent::Heartbeat => Message {
            //         type_: MessageType::Heartbeat,
            //         payload: Vec::new(),
            //     }
            //     .to_bytes()?,
            //     SocketEvent::Bytes(b) => b,
            //     _ => unimplemented!(),
            // };
            let bytes = bincode::serialize(&event)?;

            let mut len_buf = [0; 4];
            LittleEndian::write_u32(&mut len_buf, bytes.len() as u32);

            let mut stream = match &self.connections.iter().find(|(a, _)| a == &address) {
                Some(s) => &s.1 .0,
                None => {
                    self.out_sender.send((address, event));
                    continue;
                }
            };

            if let Ok(()) = stream.write_all(&len_buf) {
                stream.write_all(&bytes);
            }

            match &event {
                SocketEvent::Disconnect => {
                    let idx = self
                        .connections
                        .iter()
                        .position(|(a, _)| a == &address)
                        .unwrap();
                    let (_, (stream, _)) = &mut self.connections[idx];
                    if let Err(e) = stream.shutdown(std::net::Shutdown::Both) {
                        warn!("failed shutting down stream: {}", e);
                    }
                    self.connections.remove(idx);
                    return Ok(());
                }
                _ => (),
            }
        }

        // accept new connections, if there are any
        for stream in self.listener.incoming() {
            match stream {
                Ok(s) => {
                    //s.set_read_timeout(Some(Duration::from_millis(1)))
                    //.expect("set_read_timeout call failed");
                    s.set_nonblocking(true);
                    s.set_nodelay(true);
                    self.connections.push((s.peer_addr()?, (s, Vec::new())));
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => return Err(Error::Other(format!("{:?}", e))),
            }
        }

        Ok(())
    }

    fn match_event(bytes: &[u8], addr: &SocketAddr) -> Result<SocketEvent> {
        use num_enum::TryFromPrimitive;
        if let Some(first) = bytes.first() {
            // let msg_kind: MessageType = MessageType::try_from(first.clone())?;
            // let prefix: SocketEventPrefix = SocketEventPrefix::try_from(first.clone())?;
            // let event = match prefix {
            //     SocketEventPrefix::Connect => SocketEvent::Connect,
            //     SocketEventPrefix::Disconnect => SocketEvent::Disconnect,
            //     SocketEventPrefix::Heartbeat => SocketEvent::Heartbeat,
            //     SocketEventPrefix::Timeout => SocketEvent::Timeout,
            //     SocketEventPrefix::Bytes => SocketEvent::Bytes(bytes.to_vec()),
            // };
            // Ok(event)
            unimplemented!()
        } else {
            Err(Error::Other("match_event: no bytes in buffer".to_string()))
        }
    }

    pub fn connections(&self) -> Vec<SocketAddr> {
        self.connections.iter().map(|(a, _)| *a).collect()
    }
}

//impl ConnectionHandler {
//pub fn new(addr: &str) -> Result<Self> {
//let (event_sender, event_receiver) = unbounded();
//let (user_event_sender, user_event_receiver) = unbounded();

//Ok(Self {
//listener: TcpListener::bind(addr)?,
//stream: None,
//connections: Default::default(),
//receive_buffer: Default::default(),
//event_receiver,
//user_event_sender,
//user_event_receiver,
//})
//}

///// Processes any inbound/outbound packets and events.
///// Processes connection specific logic for active connections.
///// Removes dropped connections from active connections list.
//pub fn manual_poll(&mut self) {
////let mut unestablished_connections = self.unestablished_connection_count();
////let messenger = &mut self.messenger;

//// first we pull all newly arrived messages
//for (addr, stream) in &self.connections {
//let prefix = stream.read_u8();
//}
////loop {
////if let Some(stream) = &self.stream {
//////
////}
////{
////Ok((payload, address)) => {
////if let Some(conn) = self.connections.get_mut(&address) {
////let was_est = conn.is_established();
////conn.process_packet(messenger, payload, time);
////if !was_est && conn.is_established() {
////unestablished_connections -= 1;
////}
////} else {
////let mut conn = TConnection::create_connection(messenger, address, time);
////conn.process_packet(messenger, payload, time);

////// We only allow a maximum amount number of unestablished connections to bet created
////// from inbound packets to prevent packet flooding from allocating unbounded memory.
////if unestablished_connections < self.max_unestablished_connections as usize {
////self.connections.insert(address, conn);
////unestablished_connections += 1;
////}
////}
////}
////Err(e) => {
////if e.kind() != std::io::ErrorKind::WouldBlock {
////error!("Encountered an error receiving data: {:?}", e);
////}
////break;
////}
////}
////// prevent from blocking, break after receiving first packet
////if messenger.socket.is_blocking_mode() {
////break;
////}
////}

////// now grab all the waiting packets and send them
////while let Ok(event) = self.user_event_receiver.try_recv() {
////// get or create connection
////let conn = self.connections.entry(event.address()).or_insert_with(|| {
////TConnection::create_connection(messenger, event.address(), time)
////});

////let was_est = conn.is_established();
////conn.process_event(messenger, event, time);
////if !was_est && conn.is_established() {
////unestablished_connections -= 1;
////}
////}

////// update all connections
////for conn in self.connections.values_mut() {
////conn.update(messenger, time);
////}

////// iterate through all connections and remove those that should be dropped
////self.connections
////.retain(|_, conn| !conn.should_drop(messenger, time));
//}
//}
