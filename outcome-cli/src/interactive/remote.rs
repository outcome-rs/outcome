use crate::interactive::Config;
use outcome::Address;
use outcome_net::Client;

pub fn process_step(client: &mut Client, config: &Config) -> Result<(), String> {
    client
        .server_step_request(config.turn_ticks as u32)
        .unwrap();

    Ok(())
}

/// Create the prompt string. It defaults to current clock tick integer number.
/// It can display a custom prompt based on the configuration file.
///
/// ### Example
///
/// example of custom prompt setup (interactive.yaml):
///
/// ```yaml
/// prompt_format: "{}-{}-{} {}:00"
/// prompt_vars: [
/// 	"/universal/universal/generic/clock/string/day",
/// 	"/universal/universal/generic/clock/string/month",
/// 	"/universal/universal/generic/clock/string/year",
/// 	"/universal/universal/generic/clock/string/hour",
/// ]
/// ```
pub fn create_prompt(client: &mut Client, cfg: &Config) -> Result<String, String> {
    //    unimplemented!();
    if &cfg.prompt_format == "" {
        return create_prompt_default(client);
    }
    //    return "err".to_string();
    // vars resolved
    let mut var_addrs = Vec::new();
    for v in &cfg.prompt_vars {
        let addr = match Address::from_str(v) {
            Ok(a) => a,
            Err(e) => {
                //                println!("failed making addr from: {}", v.clone());
                return create_prompt_default(client);
            }
        };
        var_addrs.push(addr.to_string());
    }
    let vars = client.get_vars_as_strings(&var_addrs).unwrap();
    let matches: Vec<&str> = cfg.prompt_format.matches("{}").collect();
    if matches.len() != vars.len() {
        return create_prompt_default(client);
    }
    let mut out_string = format!("[{}] ", cfg.prompt_format.clone());
    for var_res in vars {
        out_string = out_string.replacen("{}", &var_res, 1);
    }
    Ok(out_string)
}
pub fn create_prompt_default(client: &mut Client) -> Result<String, String> {
    let status = client.server_status().unwrap();
    let clock = status.get("current_tick").unwrap();
    Ok(format!(
        "[{}] ",
        //TODO this should instead ask for a default clock variable
        // that's always available (right now it's using clock mod's var)
        clock,
    ))
}

fn print_show(client: &mut Client, config: &Config) {
    let mut longest_addr: usize = 0;
    for addr_str in &config.show_list {
        if addr_str.len() > longest_addr {
            longest_addr = addr_str.len();
        }
    }

    for addr_str in &config.show_list {
        // slightly convoluted way of getting two neat columns
        let len_diff = longest_addr - addr_str.len() + 6;
        let mut v = Vec::new();
        for i in 0..len_diff {
            v.push(' ')
        }
        let diff: String = v.into_iter().collect();

        // TODO
        let addr = match Address::from_str(addr_str) {
            Ok(a) => a,
            Err(_) => continue,
        };
        let val = client.get_var_as_string(&addr.to_string()).unwrap();

        println!("{}{}{}", addr_str, diff, val);
    }
}
