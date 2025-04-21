use std::fmt::Write;

use anyhow::Result;
use dc_core::{ElementInfo, MsgType};

pub fn show_element_info(info: &ElementInfo, markdown: bool) {
    let s = to_markdown(info).unwrap();
    if markdown {
        print!("{}", s);
    } else {
        termimad::print_text(&s);
    }
}

fn to_markdown(info: &ElementInfo) -> Result<String> {
    let mut s = String::new();

    writeln!(s, "# {}", info.id)?;
    writeln!(s)?;

    writeln!(s, "## Origin")?;
    writeln!(s, "{}", info.origin)?;
    writeln!(s)?;

    writeln!(s, "## Description")?;
    writeln!(s, "{}", change_heading_level(&info.description)?)?;

    writeln!(s, "## Receive Ports")?;
    if info.recv_ports == 0 {
        writeln!(s, "None")?;
    } else {
        for i in 0..info.recv_ports {
            writeln!(
                s,
                "{}. {}",
                i,
                type_array(info.recv_msg_types.get(i as usize))?
            )?;
        }
    }
    writeln!(s)?;

    writeln!(s, "## Send Ports")?;
    if info.send_ports == 0 {
        writeln!(s, "None")?;
    } else {
        for i in 0..info.send_ports {
            writeln!(
                s,
                "{}. {}",
                i,
                type_array(info.send_msg_types.get(i as usize))?
            )?;
        }
    }
    writeln!(s)?;

    writeln!(s, "## Metadata Ids")?;
    if info.metadata_ids.is_empty() {
        writeln!(s, "None")?;
    } else {
        for (i, id) in info.metadata_ids.iter().enumerate() {
            if i != 0 {
                write!(s, ", ")?;
            }
            write!(s, "{}", id)?;
        }
    }
    writeln!(s)?;

    writeln!(s, "## Configuration")?;
    writeln!(s, "{}", change_heading_level(&info.config_doc)?)?;

    Ok(s)
}

fn change_heading_level(input: &str) -> Result<String> {
    let mut s = String::new();

    for line in input.lines() {
        if line.starts_with("#") {
            writeln!(s, "##{}", line)?;
        } else {
            writeln!(s, "{}", line)?;
        }
    }

    Ok(s)
}

fn type_array(types: Option<&Vec<MsgType>>) -> Result<String> {
    let types = if let Some(types) = types {
        types
    } else {
        return Ok("any".into());
    };

    if types.is_empty() {
        return Ok("any".into());
    }

    let mut s = String::new();

    for (i, t) in types.iter().enumerate() {
        if i != 0 {
            write!(s, ", ")?;
        }
        write!(s, "{}", t)?;
    }

    Ok(s)
}
