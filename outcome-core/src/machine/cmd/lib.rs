extern crate libloading;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use self::libloading::{Library, Symbol};
use serde_yaml::Value;

use crate::address::Address;
use crate::component::Component;
use crate::entity::{Entity, Storage};
use crate::machine::cmd::{Command, CommandResult};
use crate::model::SimModel;
use crate::{model, util};
use crate::{Sim, VarType};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LibCallSign {
    Void,
    VoidArg(VarType),
    VoidArgArg(VarType, VarType),
    Ret(VarType),
    RetArg(VarType, VarType),
    RetArgArg(VarType, VarType, VarType),
    Var(VarType),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibCall {
    lib: String,
    func_name: String,
    func_signature: LibCallSign,
    args: Vec<String>,
    pipe_out: Option<Address>,
}
impl LibCall {
    pub fn from_str(mut args_str: &str) -> Result<Self, String> {
        // first separate the pipe_out ending, if there is a pipe
        // present
        let mut pipe_out = None;
        if args_str.contains("|") {
            let split: Vec<&str> = args_str.splitn(2, "|").collect::<Vec<&str>>();
            args_str = split[0].trim();
            let pipe_addr = Address::from_str_with_context(split[1].trim(), None, None).unwrap();
            pipe_out = Some(pipe_addr);
        }
        let split: Vec<&str> = args_str.split(" ").collect();
        let sign = split[1].trim();
        let sign_split: Vec<&str> = sign.split(".").collect::<Vec<&str>>();
        let mut signature = LibCallSign::Void;
        let mut ret = None;
        let mut vt1 = None;
        let mut vt2 = None;
        let mut args = Vec::new();
        match sign_split[0] {
            "fn" => {
                ret = None;
            }
            "var" => {}
            _ => {
                if sign_split[0].starts_with("fn->") {
                    ret = VarType::from_str(sign_split[0].split("->").collect::<Vec<&str>>()[1]);
                }
            }
            //            "var" => match sign_split
            _ => (),
        }
        match sign_split.get(1) {
            Some(one) => {
                vt1 = VarType::from_str(one);
                match sign_split.get(2) {
                    Some(two) => {
                        vt2 = VarType::from_str(two);
                    }
                    None => (),
                }
            }
            None => (),
        }
        signature = match ret {
            None => match vt1 {
                Some(v1) => match vt2 {
                    Some(v2) => LibCallSign::VoidArgArg(v1, v2),
                    None => LibCallSign::VoidArg(v1),
                },
                None => LibCallSign::Void,
            },
            Some(r) => match vt1 {
                Some(v1) => match vt2 {
                    Some(v2) => LibCallSign::RetArgArg(r, v1, v2),
                    None => LibCallSign::RetArg(r, v1),
                },
                None => LibCallSign::Ret(r),
            },
        };

        Ok(LibCall {
            lib: split[0].to_string(),
            func_name: split[2].to_string(),
            func_signature: signature,
            args,
            pipe_out,
        })
    }
    pub fn from_map(map: &HashMap<String, Value>) -> Result<Self, String> {
        unimplemented!()
    }
}
impl LibCall {
    pub fn execute_loc(&self, libs: &HashMap<String, Library>, es: &mut Storage) -> CommandResult {
        //        let lock = libs.try_lock().expect("failed to lock
        // arcmut");
        let lib = libs.get(&self.lib).expect("failed to get lib from lock");
        //        println!("{:?}", self.func_signature);
        unsafe {
            // try getting symbol from lib
            match self.func_signature {
                LibCallSign::Void => {
                    let func: libloading::Symbol<unsafe extern "C" fn()> =
                        match lib.get(self.func_name.as_bytes()) {
                            Ok(f) => f,
                            Err(e) => panic!("{}", e),
                        };
                    func();
                }
                LibCallSign::VoidArg(arg_vt) => {
                    match arg_vt {
                        VarType::IntGrid => {
                            unimplemented!();
                            //                            let func: libloading::Symbol<unsafe extern fn(&mut Vec<Vec<i32>>)> = match lib.get(self.func_name.as_bytes()) {
                            //                                Ok(f) => f,
                            //                                Err(e) => panic!("{}", e),
                            //                            };
                            //                            let uid = es.get_ref("map/regions/int_grid/main").expect("failed getting ref").uid;
                            //                            let mut grid = es.int_grid.get_mut(&uid);
                            //                            if !grid.is_some() {
                            //                                return CommandResult::Ok;
                            //                            }
                            //                            func(&mut grid.unwrap());
                            ////                            println!("called func VoidArg")
                        }
                        _ => (),
                    }
                }
                LibCallSign::Ret(ret_vt) => {
                    match ret_vt {
                        VarType::Int => {
                            let func: libloading::Symbol<unsafe extern "C" fn() -> i32> =
                                match lib.get(self.func_name.as_bytes()) {
                                    Ok(f) => f,
                                    Err(e) => panic!("{}", e),
                                };
                            let int: i32 = func();
                            //                            let ref_ =
                            // comp.loc_vars.get(self.pipe_out.unwrap()).unwrap();
                            *es.get_int_mut(&self.pipe_out.unwrap().get_storage_index())
                                .unwrap() = int;
                        }
                        _ => (),
                    }

                    //                    println!("{}",
                    // int);
                }
                _ => (),
            };
        }
        //        unsafe {
        ////            let func: libloading::Symbol<unsafe extern
        //// fn(&mut Vec<Vec<i32>>)> =
        //// lib.get(self.func.as_bytes()).unwrap();
        //            let func: libloading::Symbol<unsafe extern
        // fn(&mut Vec<Vec<i32>>)> = lib.get(self.func_name.
        // as_bytes())                .expect("failed
        // getting func symbol");
        ////            let uid =
        //// es.get_ref("prop/area/int_grid/ig").expect("failed
        //// getting ref").uid;
        //            let uid =
        // es.get_ref("map/regions/int_grid/main").expect("failed
        // getting ref").uid;            let mut grid =
        // es.int_grid.get_mut(&uid);            if !grid.is_some()
        // {                return CommandResult::Ok;
        //            }
        //            func(&mut grid.unwrap());
        //        }
        //        unsafe {
        //            let func: libloading::Symbol<unsafe extern
        // fn()> = lib.get(self.func.as_bytes()).unwrap();
        // func();        }
        CommandResult::Continue
    }
}
