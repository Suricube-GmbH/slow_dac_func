use cube_library_development_toolbox::*;
use extism_pdk::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::mpsc::channel;

#[host_fn]
extern "ExtismHost" {
    fn function_execute(request: String) -> u32;
    fn write_topic(file_name : String, text : String) -> u32;
    fn write_append_topic(file_name : String, text : String) -> u32; 
    fn read_topic(file_name: String) -> String;
    fn send_message_to_user(message: String) -> u32;
    fn update_parameters(actor_name: String, parameters_json: String) -> u32;
    //fn debug_print(message: String) -> (); // Does not work, make the wasm file unusable.
}

#[derive(Debug, Serialize, Deserialize)]
struct ChannelSpecifications {
    pub max_voltage: f64,
    pub min_voltage: f64,
    pub physical_index: String
}

#[plugin_fn]
pub fn update(input: String) -> FnResult<String> {
    let argument_input_json: BTreeMap<String, ParametersType> = arguments_json_to_map(&input);
    let mut parameter_input_json: BTreeMap<String, ParametersType> = parameters_json_to_map(&input);
    let actor_name: String = get_actor_name(&input);

    if let Some(ParametersType::String(channel_selected)) =
        argument_input_json.get("actor_channel_selected")
    {
        let (tx, rx) = channel::<ChannelSpecifications>();
        unsafe {
            // Slow dac channel parameters load request
            let is_json_read_string: Result<String, Error> =
                read_topic(format!("{}.json", channel_selected));
            let json_read_string: String = match is_json_read_string {
                Ok(json_read_string) => json_read_string,
                _ => {
                    return Ok(message_to_user(
                        "Error - Failed to read the file.",
                    ))
                }
            };

            // Parse the channel data
            let is_channel_data = serde_json::from_str::<ChannelSpecifications>(&json_read_string);
            let channel_data: ChannelSpecifications = match is_channel_data {
                Ok(channel_data) => channel_data,
                Err(err) => {
                    _ = send_message_to_user(json_read_string.clone()).unwrap();
                    return Ok(message_to_user(&format!(
                        "Error : Failed to load channel specs.\n{json_read_string}\n{err}"
                    )));
                }
            };

            let is_send = tx.send(channel_data);
            match is_send {
                Err(err) => {
                    return Ok(message_to_user(&format!(
                        "Error : Failed to load channel specs.\n{json_read_string}\n{err}"
                    )))
                }
                _ => {}
            }
        }

        let is_channel_data: Result<ChannelSpecifications, std::sync::mpsc::RecvError> = rx.recv();
        let channel_data = match is_channel_data {
            Ok(channel_data) => channel_data,
            Err(err) => {
                return Ok(message_to_user(&format!(
                    "Error : Failed to load channel specs.\n{err}"
                )))
            }
        };

        // Value calculation
        if let Some(ParametersType::u64(raw_value)) = argument_input_json.get("raw_value") {
            parameter_input_json
                .insert("value".to_string(), ParametersType::u32(*raw_value as u32));
        } else if let Some(voltage_value_parameter_type) = argument_input_json.get("voltage_value")
        {
            let u16_range_f64: f64 = (u16::MAX - u16::MIN)
                .try_into()
                .expect("Should be able to convert from u16 to f64");
            match voltage_value_parameter_type {
                ParametersType::u64(voltage_value) => {
                    let value_f64: f64 = (*voltage_value as f64 - channel_data.min_voltage)
                        * (u16_range_f64)
                        / (channel_data.max_voltage - channel_data.min_voltage);
                    let value_rounded: f64 = value_f64.round();
                    parameter_input_json.insert(
                        "value".to_string(),
                        ParametersType::u32(value_rounded as u32),
                    );
                }
                ParametersType::f64(voltage_value) => {
                    let value_f64: f64 = (voltage_value - channel_data.min_voltage)
                        * (u16_range_f64)
                        / (channel_data.max_voltage - channel_data.min_voltage);
                    let value_rounded: f64 = value_f64.round();
                    parameter_input_json.insert(
                        "value".to_string(),
                        ParametersType::u32(value_rounded as u32),
                    );
                }
                _ => {
                    return Ok(message_to_user(
                        "Error - There is problem in your value linked function parameters."
                    ));
                }
            }
        } else {
            return Ok(message_to_user(
                "Error - There is problem in your value linked function parameters.",
            ));
        }

        let dac_channel_code = match channel_data.physical_index.as_str() {
            "A" => 0,
            "B" => 1,
            "C" => 2,
            "D" => 3,
            "E" => 4,
            "F" => 5,
            "G" => 6,
            "H" => 7,
            _ => {
                return Ok(message_to_user(
                    "Error - Invalid board channel index. Valid is [A-H].",
                ));
            }
        };
        parameter_input_json.insert(
            "dac_channel_code".to_string(),
            ParametersType::u32(dac_channel_code),
        );

        // Command insertion
        parameter_input_json.insert("command".to_string(), ParametersType::u32(3));

        // TX_enable insertion and first modification
        parameter_input_json.insert("tx_enable".to_string(), ParametersType::bool(true));


        let mut final_request = parameters_map_to_json(parameter_input_json);
        final_request = add_actor_function_request(&actor_name, "tx_disable", BTreeMap::new(), &final_request);

        return Ok(final_request);
    } else {
        return Ok(message_to_user(
            "Error - There is problem in your \"actor_channel_selected\", you probably forgot it."
        ));
    }
}

#[plugin_fn]
pub fn tx_disable (input: String) -> FnResult<String> {
    let mut parameter_input_json: BTreeMap<String, ParametersType> = parameters_json_to_map(&input);
    parameter_input_json.insert("tx_enable".to_string(), ParametersType::bool(false));
    let json_return = parameters_map_to_json(parameter_input_json);

    Ok(json_return)
}