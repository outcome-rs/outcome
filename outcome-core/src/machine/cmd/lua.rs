use std::collections::HashMap;
use std::error::Error;

use rlua::prelude::{LuaContext, LuaValue};
use rlua::{AnyUserData, Chunk, Context, Function, Lua, UserData, UserDataMethods};
use serde_yaml::Value;

use super::getopts::Options;
use crate::address::Address;
use crate::component::Component;
use crate::entity::{CompCollection, Entity, Storage};
use crate::machine::cmd::{Attach, ExtSet, ExtSetVar, Get, Spawn};
use crate::machine::cmd::{CentralRemoteCommand, Command, CommandResult, ExtCommand};
use crate::model::SimModel;
use crate::{model, util};
use crate::{Sim, VarType};
use crate::{StringId, Var};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LuaScript {
    inputs: HashMap<String, Address>,
    outputs: HashMap<String, Address>,
    src: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LuaCall {
    func: String,
    args: HashMap<String, Address>,
    pipe_out: Option<Address>,
    err: CommandResult,
    ok: CommandResult,
}
impl LuaCall {
    pub fn from_str(mut args_str: &str) -> Result<Self, String> {
        // first separate the pipe_out ending, if there is a pipe
        // present
        let mut pipe_out = None;
        if args_str.contains("|") {
            let split: Vec<&str> = args_str.splitn(2, "|").collect::<Vec<&str>>();
            //            println!("{:?}", split);
            args_str = split[0].trim();
            let pipe_addr = Address::from_str_with_context(split[1].trim(), None, None).unwrap();
            pipe_out = Some(pipe_addr);
        }
        let mut options = Options::new();
        options.optopt(
            "",
            "err",
            "Result for situation where the call fails",
            "RESULT",
        );
        options.optopt(
            "",
            "ok",
            "Result for situation where the call succeeds",
            "RESULT",
        );
        options.optopt("a", "args", "List of arguments (addresses)", "RESULT");
        let split = args_str.split(" ").collect::<Vec<&str>>();
        let matches = match options.parse(&split) {
            Ok(m) => m,
            Err(e) => return Err(e.to_string()),
        };
        let args = match matches.opt_str("args") {
            Some(opt_s) => {
                let mut out_map = HashMap::new();
                let opt_split: Vec<&str> = opt_s.split(",").collect::<Vec<&str>>();
                for (n, mut s) in opt_split.iter().enumerate() {
                    let mut key = format!("{}", n);
                    let mut val = s.to_string();
                    if s.contains("=") {
                        let key_split = s.split("=").collect::<Vec<&str>>();
                        key = key_split[0].to_owned();
                        val = key_split[1].to_owned();
                    }
                    let addr = match Address::from_str_with_context(&val, None, None) {
                        Some(a) => a,
                        None => {
                            debug!("invalid lua_call arg: {}", s);
                            continue;
                        }
                    };
                    out_map.insert(key, addr);
                }
                out_map
            }
            None => HashMap::new(),
        };
        //        println!("lua_invoke from_str()");
        Ok(LuaCall {
            func: split[0].to_string(),
            args,
            pipe_out,
            err: CommandResult::Continue,
            ok: CommandResult::Continue,
        })
    }
    fn from_map(map: &HashMap<String, Value>) -> Result<Self, String> {
        unimplemented!()
    }
    fn from_cmd(
        cmd: &Command,
        ent: &Entity,
        comp: &Component,
        model: &SimModel,
    ) -> Result<Self, String> {
        unimplemented!()
    }
    fn setup(self, ent: &Entity, comp: &Component, model: &SimModel) -> Self {
        unimplemented!()
    }
}
struct ProcHandle<'a>(
    &'a SimModel,
    &'a mut Storage,
    &'a mut CompCollection,
    &'a mut Vec<CommandResult>,
);
use std::str::FromStr;
impl<'a> UserData for ProcHandle<'a> {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method_mut("break_state", |ctx: rlua::Context, data, ()| {
            let mut cmd_res: &mut Vec<CommandResult> = &mut data.3;
            cmd_res.push(CommandResult::Break);
            Ok(())
        });
        methods.add_method_mut(
            "spawn_clone",
            |ctx: rlua::Context, data, (ent_type, ent_id, new_id): (String, String, String)| {
                let mut cmd_res: &mut Vec<CommandResult> = &mut data.3;
                cmd_res.push(CommandResult::ExecCentralExt(CentralRemoteCommand::Spawn(
                    Spawn {
                        model_type: StringId::from_str(&ent_type).unwrap(),
                        model_id: StringId::from_str(&ent_id).unwrap(),
                        spawn_id: StringId::from_str(&new_id).unwrap(),
                    },
                )));
                Ok(())
            },
        );
        methods.add_method_mut(
            "attach",
            |ctx: rlua::Context, data, (model_type, model_id, new_id): (String, String, String)| {
                let model: &SimModel = &data.0;
                let mut storage: &mut Storage = &mut data.1;
                let mut comps: &mut CompCollection = &mut data.2;
                unimplemented!();
                // comps.attach(model, storage, &model_type, &model_id, &new_id);
                // cmd_res.push(CommandResult::
                // ExecCentralExt(CentralExtCommand::Attach(
                // Attach {
                // model_type: ShortString::from_str(&model_type).unwrap(),
                // model_id: ShortString::from_str(&model_id).unwrap(),
                // new_id: ShortString::from_str(&new_id).unwrap(),
                //},
                //)));
                Ok(())
            },
        );

        methods.add_method_mut(
            "ext_get_addr",
            |ctx: rlua::Context, data, (target, source): (String, String)| {
                let target = Address::from_str(&target).unwrap();
                let source = Address::from_str(&source).unwrap();
                // println!("{:?}", address);
                let mut ext_cmds: &mut Vec<CommandResult> = &mut data.3;
                ext_cmds.push(CommandResult::ExecExt(ExtCommand::Get(Get {
                    target,
                    source,
                })));
                Ok(())
            },
        );
        methods.add_method_mut(
            "ext_set_addr",
            |ctx: rlua::Context, data, (target, source): (String, String)| {
                let target = Address::from_str(&target).unwrap();
                let source = Address::from_str(&source).unwrap();
                // println!("{:?}", address);
                let mut ext_cmds: &mut Vec<CommandResult> = &mut data.3;
                ext_cmds.push(CommandResult::ExecExt(ExtCommand::Set(ExtSet {
                    target,
                    source,
                })));
                Ok(())
            },
        );
        methods.add_method_mut(
            "ext_set",
            |ctx: rlua::Context, data, (target, val): (String, LuaValue)| {
                let target = Address::from_str(&target).unwrap();
                let vt = target.get_var_type();
                let var = match vt {
                    VarType::Int => Var::Int(ctx.unpack(val).unwrap()),
                    VarType::Bool => Var::Bool(ctx.unpack(val).unwrap()),
                    _ => unimplemented!(),
                };
                let mut ext_cmds: &mut Vec<CommandResult> = &mut data.3;
                ext_cmds.push(CommandResult::ExecExt(ExtCommand::SetVar(ExtSetVar {
                    target,
                    source: var,
                })));
                Ok(())
            },
        );
        methods.add_method_mut(
            "set",
            |ctx: rlua::Context, data, (mut uid, val): (String, LuaValue)| {
                let uid_str = match uid.starts_with("~/") {
                    true => &uid[2..],
                    false => &uid,
                };
                let ent_comps: &mut CompCollection = &mut data.2;
                let ent_storage: &mut Storage = &mut data.1;
                let loc_addr = Address::from_str(uid_str).unwrap();
                let var_euid = loc_addr.get_storage_index();
                let s = uid_str.rsplitn(3, "/").collect::<Vec<&str>>()[1];
                let vt = VarType::from_str(s).unwrap();
                match vt {
                    VarType::Str => {
                        *ent_storage.get_str_mut(&var_euid).unwrap() =
                            String::from(ctx.coerce_string(val).unwrap().unwrap().to_str().unwrap())
                    }
                    VarType::Int => {
                        *ent_storage.get_int_mut(&var_euid).unwrap() =
                            ctx.coerce_integer(val).unwrap().unwrap() as i32
                    }
                    VarType::Float => {
                        *ent_storage.get_float_mut(&var_euid).unwrap() =
                            ctx.coerce_number(val).unwrap().unwrap() as f32
                    }
                    VarType::Bool => {
                        *ent_storage.get_bool_mut(&var_euid).unwrap() = ctx.unpack(val).unwrap()
                    }
                    _ => unimplemented!(),
                    // VarType::StrList => {
                    //     *ent_storage.get_str_list_mut(&var_euid).unwrap() = ctx.unpack(val).unwrap()
                    // }
                    // VarType::IntList => {
                    //     *ent_storage.int_list.get_mut(&var_euid).unwrap() = ctx.unpack(val).unwrap()
                    // }
                    // VarType::FloatList => {
                    //     *ent_storage.float_list.get_mut(&var_euid).unwrap() =
                    //         ctx.unpack(val).unwrap()
                    // }
                    // VarType::BoolList => {
                    //     *ent_storage.bool_list.get_mut(&var_euid).unwrap() =
                    //         ctx.unpack(val).unwrap()
                    // }
                    // VarType::StrGrid => {
                    //     *ent_storage.str_grid.get_mut(&var_euid).unwrap() = ctx.unpack(val).unwrap()
                    // }
                    // VarType::IntGrid => {
                    //     *ent_storage.int_grid.get_mut(&var_euid).unwrap() = ctx.unpack(val).unwrap()
                    // }
                    // VarType::FloatGrid => {
                    //     *ent_storage.float_grid.get_mut(&var_euid).unwrap() =
                    //         ctx.unpack(val).unwrap()
                    // }
                    // VarType::BoolGrid => {
                    //     *ent_storage.bool_grid.get_mut(&var_euid).unwrap() =
                    //         ctx.unpack(val).unwrap()
                    // }
                };
                Ok(())
            },
        );
        methods.add_method("get", |ctx: rlua::Context, data, mut uid: String| {
            //            let mut uid_addr = String::new();
            let uid_str = match uid.starts_with("~/") {
                true => &uid[2..],
                false => &uid,
            };
            let ent_storage: &Storage = &*data.1;
            let loc_addr = Address::from_str(uid_str).unwrap();
            let var_euid = loc_addr.get_storage_index();
            let s = uid_str.rsplitn(3, "/").collect::<Vec<&str>>()[1];
            let vt = VarType::from_str(s).unwrap();
            match vt {
                VarType::Str => Ok(ctx
                    .pack(ent_storage.get_str(&var_euid).unwrap().clone())
                    .unwrap()),
                VarType::Int => Ok(ctx.pack(*ent_storage.get_int(&var_euid).unwrap()).unwrap()),
                VarType::Float => Ok(ctx
                    .pack(*ent_storage.get_float(&var_euid).unwrap())
                    .unwrap()),
                VarType::Bool => Ok(ctx.pack(*ent_storage.get_bool(&var_euid).unwrap()).unwrap()),
                _ => unimplemented!(),
                // VarType::StrList => Ok(ctx
                //     .pack(ent_storage.str_list.get(&var_euid).unwrap().clone())
                //     .unwrap()),
                // VarType::IntList => Ok(ctx
                //     .pack(ent_storage.int_list.get(&var_euid).unwrap().clone())
                //     .unwrap()),
                // VarType::FloatList => Ok(ctx
                //     .pack(ent_storage.float_list.get(&var_euid).unwrap().clone())
                //     .unwrap()),
                // VarType::BoolList => Ok(ctx
                //     .pack(ent_storage.bool_list.get(&var_euid).unwrap().clone())
                //     .unwrap()),
                // VarType::StrGrid => Ok(ctx
                //     .pack(ent_storage.str_grid.get(&var_euid).unwrap().clone())
                //     .unwrap()),
                // VarType::IntGrid => Ok(ctx
                //     .pack(ent_storage.int_grid.get(&var_euid).unwrap().clone())
                //     .unwrap()),
                // VarType::FloatGrid => Ok(ctx
                //     .pack(ent_storage.float_grid.get(&var_euid).unwrap().clone())
                //     .unwrap()),
                // VarType::BoolGrid => Ok(ctx
                //     .pack(ent_storage.bool_grid.get(&var_euid).unwrap().clone())
                //     .unwrap()),
            }
            //            Ok(())
        });
    }
}

impl LuaCall {
    pub fn execute_loc_lua(
        &self,
        // lua_state: &mut Option<Lua>,
        // storage: &mut RamStorage,
        model: &SimModel,
        entity: &mut Entity,
    ) -> Vec<CommandResult> {
        let lua_state = &mut entity.insta.lua_state;
        let storage = &mut entity.storage;
        let comps = &mut entity.components;
        let mut out_cmds = Vec::new();
        let mut ok = true;
        let lua_state = match lua_state {
            Some(ls) => ls,
            None => {
                debug!("lua_state is none");
                return vec![self.err.clone()];
            }
        };
        lua_state.lock().unwrap().context(|ctx| {
            let mut res_val = rlua::Value::Nil;
            let globals = ctx.globals();
            ctx.scope(|scope| {
                //                let userdata =
                // scope.create_nonstatic_userdata(EntityUserdata(storage)).
                // unwrap();

                // create the args table
                let mut args_lua = ctx.create_table().unwrap();
                for (arg_key, arg_addr) in &self.args {
                    match arg_addr.var_type.unwrap() {
                        VarType::Str => args_lua
                            .set(
                                arg_key.to_owned(),
                                ctx.pack(
                                    storage
                                        .get_str(&arg_addr.get_storage_index())
                                        .unwrap()
                                        .to_owned(),
                                )
                                .unwrap(),
                            )
                            .unwrap(),
                        VarType::Int => args_lua
                            .set(
                                arg_key.to_owned(),
                                ctx.pack(*storage.get_int(&arg_addr.get_storage_index()).unwrap())
                                    .unwrap(),
                            )
                            .unwrap(),
                        VarType::Float => args_lua
                            .set(
                                arg_key.to_owned(),
                                ctx.pack(
                                    *storage.get_float(&arg_addr.get_storage_index()).unwrap(),
                                )
                                .unwrap(),
                            )
                            .unwrap(),
                        VarType::Bool => args_lua
                            .set(
                                arg_key.to_owned(),
                                ctx.pack(*storage.get_bool(&arg_addr.get_storage_index()).unwrap())
                                    .unwrap(),
                            )
                            .unwrap(),
                        _ => unimplemented!(),
                    }
                }

                // create the userdata object
                let userdata = scope
                    .create_nonstatic_userdata(ProcHandle(model, storage, comps, &mut out_cmds))
                    .unwrap();

                // get the function
                match globals.get(self.func.as_str()) {
                    Ok(f) => {
                        let func: Function = f;
                        res_val = match func.call::<_, rlua::Value>((userdata, args_lua)) {
                            Ok(v) => v,
                            Err(e) => {
                                error!("lua_call: {}", e);
                                ok = false;
                                rlua::Value::Nil
                            }
                        };
                    }
                    Err(e) => {
                        debug!("{}: func name: {}", e, self.func);
                        ok = false;
                    }
                };

                //                println!("{:?}", res_val);
            });
            match self.pipe_out {
                Some(addr) => {
                    match addr.var_type.unwrap() {
                        VarType::Str => match ctx.unpack(res_val) {
                            Ok(v) => *storage.get_str_mut(&addr.get_storage_index()).unwrap() = v,
                            Err(e) => error!("pipe failed: {}", e),
                        },
                        VarType::Int => match ctx.unpack(res_val) {
                            Ok(v) => *storage.get_int_mut(&addr.get_storage_index()).unwrap() = v,
                            Err(e) => error!("pipe failed: {}", e),
                        },
                        VarType::Float => match ctx.unpack(res_val) {
                            Ok(v) => *storage.get_float_mut(&addr.get_storage_index()).unwrap() = v,
                            Err(e) => error!("pipe failed: {}", e),
                        },
                        VarType::Bool => match ctx.unpack(res_val) {
                            Ok(v) => *storage.get_bool_mut(&addr.get_storage_index()).unwrap() = v,
                            Err(e) => error!("pipe failed: {}", e),
                        },
                        _ => unimplemented!(),
                        // VarType::StrList => match ctx.unpack(res_val) {
                        //     Ok(v) => *storage.str_list.get_mut(&addr.get_var_euid()).unwrap() = v,
                        //     Err(e) => error!("pipe failed: {}", e),
                        // },
                        // VarType::IntList => match ctx.unpack(res_val) {
                        //     Ok(v) => *storage.int_list.get_mut(&addr.get_var_euid()).unwrap() = v,
                        //     Err(e) => error!("pipe failed: {}", e),
                        // },
                        // VarType::FloatList => match ctx.unpack(res_val) {
                        //     Ok(v) => *storage.float_list.get_mut(&addr.get_var_euid()).unwrap() = v,
                        //     Err(e) => error!("pipe failed: {}", e),
                        // },
                        // VarType::BoolList => match ctx.unpack(res_val) {
                        //     Ok(v) => *storage.bool_list.get_mut(&addr.get_var_euid()).unwrap() = v,
                        //     Err(e) => error!("pipe failed: {}", e),
                        // },
                        // VarType::StrGrid => match ctx.unpack(res_val) {
                        //     Ok(v) => *storage.str_grid.get_mut(&addr.get_var_euid()).unwrap() = v,
                        //     Err(e) => error!("pipe failed: {}", e),
                        // },
                        // VarType::IntGrid => match ctx.unpack(res_val) {
                        //     Ok(v) => *storage.int_grid.get_mut(&addr.get_var_euid()).unwrap() = v,
                        //     Err(e) => error!("pipe failed: {}", e),
                        // },
                        // VarType::FloatGrid => match ctx.unpack(res_val) {
                        //     Ok(v) => *storage.float_grid.get_mut(&addr.get_var_euid()).unwrap() = v,
                        //     Err(e) => error!("pipe failed: {}", e),
                        // },
                        // VarType::BoolGrid => match ctx.unpack(res_val) {
                        //     Ok(v) => *storage.bool_grid.get_mut(&addr.get_var_euid()).unwrap() = v,
                        //     Err(e) => error!("pipe failed: {}", e),
                        // },
                    };
                }
                None => (),
            }
        });
        // TODO remove
        lua_state.lock().unwrap().gc_collect();
        if !ok {
            out_cmds.push(self.err.clone());
        } else {
            out_cmds.push(self.ok.clone());
        }
        // println!("{:?}", out_cmds);
        out_cmds
    }
}
