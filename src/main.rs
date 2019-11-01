//
// edc2svd
// Converts an MCU register description from the EDC format to the SVD format
//
// Copyright (c) 2019 Kiffie van Haash
//
// SPDX-License-Identifier: MIT
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to
// deal in the Software without restriction, including without limitation the
// rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
// sell copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS
// IN THE SOFTWARE.
//

use getopts::Options;
use std::env;
use std::fs::File;

use log::{info, warn};
use xmltree::{Element, EmitterConfig};


fn print_usage(program: &str, opts: Options) {
    let brief = format!("\nUsage: {} [options] <input.edc> <output.svd>", program);
    print!("{}", opts.usage(&brief));
}

fn parse_u32(text: &str) -> Result<u32, std::num::ParseIntError> {
    if text.starts_with("0x") {
        u32::from_str_radix(&text[2..], 16)
    }else{
        u32::from_str_radix(&text, 10)
    }
}

fn add_elem_with_text(parent: &mut Element, name: &str, text: &str){
    let mut elem = Element::new(name);
    elem.text = Some(text.to_string());
    parent.children.push(elem);
}

fn add_register(peri_out_e: &mut Element,
                name: &str,
                offset: u32,
                reset_val: u32,
                sfrmode_e: &Element)
{
    let peri_e = &mut peri_out_e.children.last_mut().unwrap();
    let registers = &mut peri_e.children.last_mut().unwrap();

    let mut reg_e = Element::new("register");
    add_elem_with_text(&mut reg_e, "name", name);
    add_elem_with_text(&mut reg_e, "description", &format!("{} register", name));
    add_elem_with_text(&mut reg_e, "addressOffset", &format!("0x{:0x}", offset));
    add_elem_with_text(&mut reg_e, "size", "32");
    add_elem_with_text(&mut reg_e, "resetValue", &format!("{}", reset_val));

    // add field descriptions if any
    let mut fields_e = Element::new("fields");
    let mut bitpos = 0;
    for elem in sfrmode_e.children.iter() {
        if elem.name == "SFRFieldDef" {
            let fname = &elem.attributes["cname"];
            if fname != &elem.attributes["name"] {
                warn!("cname = {} but name = {}", fname, &elem.attributes["name"]);
            }
            let width = parse_u32(&elem.attributes["nzwidth"]).unwrap();
            info!("\t\t[{}:{}]\t{}", bitpos + width - 1, bitpos, fname);
            let mut field_e = Element::new("field");
            add_elem_with_text(&mut field_e, "name", &fname);
            add_elem_with_text(&mut field_e,
                               "bitRange",
                               &format!("[{}:{}]", bitpos + width - 1, bitpos));
            fields_e.children.push(field_e);
            bitpos += width;
        }else if elem.name == "AdjustPoint" {
            let offset = parse_u32(&elem.attributes["offset"]).unwrap();
            bitpos += offset;
        }else {
            panic!(format!("unexpected element {} in field definition", elem.name));
        }
    }
    if bitpos > 0 {
        reg_e.children.push(fields_e);
    }
    registers.children.push(reg_e);
}

fn analyze_periph(periph: &Element, periph_out_e: &mut Element) {
    let mut peri = String::new();
    let mut base_addr: u32 = 0;
    for child in periph.children.iter() {
        if child.name == "SFRDef" {
            let attr = &child.attributes;
            // get the phys. address and map it to the KSEG1 segment
            let addr = parse_u32(&attr["_addr"]).unwrap() | 0xA000_0000;

            let name = &attr["name"];
            assert_eq!(name, &attr["cname"]);

            let mut portals = String::from("- - -");
            if let Some(p) = attr.get("portals") {
                portals = p.to_string();
            }

            let (clr, set, inv) = match portals.as_ref() {
                "CLR SET INV" => (true, true, true),
                "CLR - -" => (true, false, false),
                "- - -" => (false, false, false),
                _ => panic!(format!("unexpected portals attribute: {}", portals)),
            };

            // get reset value; map unimplemented (-) or undefined (x) bits to 0
            let reset_str = attr["mclr"]
                .replace("-", "0")
                .replace("x", "0")
                .replace("u", "0");
            let reset = u32::from_str_radix(&reset_str, 2).unwrap_or_else(|_|{
                panic!("cannot parse mclr attribute string \"{}\"", attr["mclr"]);
            });

            // guess peripheral
            let mop = match attr.get("memberofperipheral") {
                Some(m) => if m.len() == 0 { None } else { Some(m) },
                None => None,
            };
            let mut cperi: String;
            if let Some(bop) = attr.get("baseofperipheral") {
                cperi = bop.clone();
            } else if let Some(m) = mop {
                cperi = m.clone();
            } else if let Some(grp) = attr.get("grp") {
                cperi = grp.clone();
            } else if let Some(ms) = attr.get("_modsrc") {
                cperi = if ms == "DOS-01618_RPINRx.Module" ||
                           ms == "DOS-01618_RPORx.Module"  ||
                           ms == "DOS-01423_RPINRx.Module" ||
                           ms == "DOS-01423_RPORx.Module"
                {
                    String::from("PPS")
                }else if ms == "DOS-01475_lpwr_deep_sleep_ctrl_v2.Module" {
                    String::from("DSCTRL") // Deep Sleep Controller
                } else {
                    String::from("")
                };
            } else {
                panic!(format!("missing peripheral for {}", name));
            }
            let words: Vec<&str> = cperi.split(' ').collect();
            if let Some(word) = words.get(0) {
                cperi = word.to_string();
            }
            if cperi.len() == 0 {
                panic!(format!("empty peripheral info for {}", name));
            }

            // find first field definition
            let modelist_e = child.get_child("SFRModeList").unwrap();
            let sfrmode_e = modelist_e.get_child("SFRMode").unwrap();

            if cperi != peri {
                assert!(base_addr < addr); // not strictly needed
                base_addr = addr;
                peri = cperi;
                let mut peri_e = Element::new("peripheral");
                let mut name_e = Element::new("name");
                name_e.text = Some(peri.clone());
                let mut desc_e = Element::new("description");
                desc_e.text = Some(format!("{} peripheral", peri));
                let mut base_addr_e = Element::new("baseAddress");
                base_addr_e.text = Some(format!("0x{:0x}", base_addr));
                let registers_e = Element::new("registers");
                peri_e.children.push(name_e);
                peri_e.children.push(desc_e);
                peri_e.children.push(base_addr_e);
                peri_e.children.push(registers_e);
                periph_out_e.children.push(peri_e);
                info!("{} base_addr = {:0x}", peri, base_addr);
            }
            assert!(base_addr <= addr);
            let offset = addr - base_addr;
            info!("  {}", name);
            info!("\t{}   : {:0x}, offset = {:0x}, reset = {:0x} ({})",
                   name,
                   addr, offset, reset,
                   portals);
            add_register(periph_out_e, name, offset, reset, sfrmode_e);
            if clr {
                info!("\t{}CLR: {:0x}, offset = {:0x}",
                    name,
                    addr + 0x4, offset + 0x04);
                // use 0 as reset value; read from this register is undefined
                add_register(periph_out_e, &format!("{}CLR", name), offset + 4, 0, sfrmode_e);
            }
            if set {
                info!("\t{}SET: {:0x}, offset = {:0x}",
                    name,
                    addr + 0x8, offset + 8);
                // use 0 as reset value; read from this register is undefined
                add_register(periph_out_e, &format!("{}SET", name), offset + 8, 0, sfrmode_e);
            }
            if inv {
                info!("\t{}INV: {:0x}, offset = {:0x}",
                    name,
                    addr + 0xc, offset + 0xc);
                // use 0 as reset value; read from this register is undefined
                add_register(periph_out_e, &format!("{}INV", name), offset + 0xc, 0, sfrmode_e);
            }
            info!("");
        }
    }
}

fn setup_logger(loglevel: log::LevelFilter) {
    fern::Dispatch::new()
        .level(log::LevelFilter::Info)
        .level_for(module_path!(), loglevel)
        .chain(std::io::stdout())
        .apply()
        .unwrap();
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let program = "edc2svd";
    let mut opts = Options::new();
    opts.optflag("h", "help", "show this help message");
    opts.optflag("v", "verbose", "activate verbose output");

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(_) => {
            print_usage(&program, opts);
            return;
        }
    };
    setup_logger(if matches.opt_present("v") {
        log::LevelFilter::Info
    } else {
        log::LevelFilter::Error
    });

    if matches.opt_present("h") || matches.free.len() != 2 {
        print_usage(&program, opts);
        return;
    }
    let (edcfn, svdfn) = (&matches.free[0], &matches.free[1]);

    let infile = File::open(&edcfn)
        .expect(&format!("cannot open file {}", edcfn));
    let docelem = Element::parse(infile).unwrap();
    let name = &docelem.attributes["name"];
    let phys = docelem
        .get_child("PhysicalSpace")
        .expect("PhysicalSpace element missing");

    let mut develem = Element::new("device");
    let mut name_e = Element::new("name");
    name_e.text = Some(name.to_string());
    develem.children.push(name_e);
    let mut periph_out = Element::new("peripherals");

    for child in phys.children.iter() {
        if child.name == "SFRDataSector" && 
           child.attributes.get("regionid").unwrap_or(&String::from("")).starts_with("periph")
        {
            analyze_periph(child, &mut periph_out);
        }
    }
    let outfile = File::create(&svdfn).expect(&format!("cannot open file {}", svdfn));
    let config = EmitterConfig::new().perform_indent(true);
    develem.children.push(periph_out);
    develem.write_with_config(outfile, config).unwrap();
}
