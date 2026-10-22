//! The AM13E2x families.
//!
//! The AM13E2x are Cortex-M33 MCUs built from MSPM0 peripherals (GPIO, IOMUX, GPTIMER, DMA,
//! UNICOMM, WWDT) next to C2000 control peripherals (MCPWM, ECAP, EQEP, CMPSS). The peripherals
//! the two families share have the same register blocks, but the AM13 sources are laid out
//! differently, so they are read here instead of by the MSPM0 path:
//!
//! - `am13e2x-sdk` `hw_memmap.h` and `hw_ints.h`: peripheral addresses and interrupts.
//! - `am13e2x-sdk` `hw_pinmap.h`: the PINCM index of every pin and its pin functions (PF). The
//!   SysConfig metadata of these parts carries neither.
//! - SysConfig `<FAMILY>.json`: packages and their pinouts.
//!
//! Interrupts are not grouped: the M33 NVIC has a line for every peripheral.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    sync::LazyLock,
};

use anyhow::{bail, Context};
use mspm0_data_types::{
    Chip, DmaChannel, Interrupt, Package, PackagePin, Peripheral, PeripheralPin, PeripheralType,
    PowerDomain,
};
use regex::Regex;
use serde::Deserialize;

use crate::{
    generate::{adc_vrsel_mapping, convert_memory},
    header::Header,
    parts::PartFamily,
    verify,
};

pub fn is_am13(family: &str) -> bool {
    family.starts_with("am13")
}

/// A peripheral of the family: metadata name, `hw_memmap.h` name, type and register block.
struct Desc {
    name: &'static str,
    base: &'static str,
    ty: PeripheralType,
    version: Option<&'static str>,
}

const fn p(
    name: &'static str,
    base: &'static str,
    ty: PeripheralType,
    version: Option<&'static str>,
) -> Desc {
    Desc {
        name,
        base,
        ty,
        version,
    }
}

/// The peripherals of the AM13E230x, other than GPIO (one peripheral per port, built from the
/// pin map).
///
/// A version names the `data/registers` block. Each was checked against the AM13E230X SVD
/// (mspm0-data-sources `svd/`): `gpio_v1`, `tim_v1` and `wwdt_v1` match the AM13 blocks as they
/// are; `dma_v2`, `iomux_am13e230x`, `unicomm_v1` and `unicommuart_v1` are written for them by
/// `transforms/am13e230x.py`, and `sysctl_am13e230x` comes from `transforms/SYSCTL_AM13E230X.yaml`.
/// `None` means no register block has been curated yet.
#[rustfmt::skip]
const AM13E230X: &[Desc] = {
    use PeripheralType::*;
    &[
        p("ADC0", "ADC0", Adc, None),
        p("ADC1", "ADC1", Adc, None),
        p("ADC2", "ADC2", Adc, None),
        p("AES", "AES", Aes, None),
        p("CMPSS0", "CMPSS0", Cmpss, None),
        p("CMPSS1", "CMPSS1", Cmpss, None),
        p("CMPSS2", "CMPSS2", Cmpss, None),
        p("CMPSS3", "CMPSS3", Cmpss, None),
        p("CRC", "CRC", Crc, None),
        p("DEBUGSS", "DEBUGSS", Debugss, None),
        p("DMA", "DMA0", Dma, Some("v2")),
        p("EAM", "EAM", Eam, None),
        p("ECAP0", "ECAP0", Ecap, None),
        p("ECAP1", "ECAP1", Ecap, None),
        p("EPI0", "EPI0", Epi, None),
        p("EQEP0", "EQEP0", Eqep, None),
        p("EQEP1", "EQEP1", Eqep, None),
        p("EQEP2", "EQEP2", Eqep, None),
        p("FACTORYREGION", "FACTORYREGION", FactoryRegion, None),
        p("FRI", "FRI", Fri, None),
        p("GSC", "GSC", Gsc, None),
        p("INPUTXBAR", "INPUTXBAR", Xbar, None),
        p("IOMUX", "IOMUX", Iomux, Some("am13e230x")),
        p("KEYSTORE", "KEYSTORE", KeystoreCtl, None),
        p("MCAN0", "MCAN0", Canfd, None),
        p("MCPWM0", "MCPWM0", Mcpwm, None),
        p("MCPWM1", "MCPWM1", Mcpwm, None),
        p("MCPWM2", "MCPWM2", Mcpwm, None),
        p("MCPWM3", "MCPWM3", Mcpwm, None),
        p("MCPWM4", "MCPWM4", Mcpwm, None),
        p("MEMCFG", "MEMCFG", Memcfg, None),
        p("NVMNW", "NVMNW", FlashCtl, None),
        p("OUTPUTXBAR", "OUTPUTXBAR", Xbar, None),
        p("PGA0", "PGA0", Pga, None),
        p("PGA1", "PGA1", Pga, None),
        p("PGA2", "PGA2", Pga, None),
        p("PWMXBAR", "PWMXBAR", Xbar, None),
        p("SPG0", "SPG0", Spgss, None),
        p("SPG1", "SPG1", Spgss, None),
        p("SYNC", "SYNC", Xbar, None),
        p("SYSCTL", "SYSCTL", Sysctl, Some("am13e230x")),
        p("TIMG4", "TIMG4", Tim, Some("v1")),
        p("TIMG12", "TIMG12", Tim, Some("v1")),
        p("TINIE", "TINIE", Npu, None),
        p("UC0", "UNICOMM0", Unicomm, Some("v1")),
        p("UC1", "UNICOMM1", Unicomm, Some("v1")),
        p("UC2", "UNICOMM2", Unicomm, Some("v1")),
        p("UC3", "UNICOMM3", Unicomm, Some("v1")),
        p("UC4", "UNICOMM4", Unicomm, Some("v1")),
        p("UC5", "UNICOMM5", Unicomm, Some("v1")),
        p("UC0_I2CC", "UC0_I2CC", UnicommI2cc, None),
        p("UC1_I2CC", "UC1_I2CC", UnicommI2cc, None),
        p("UC2_I2CC", "UC2_I2CC", UnicommI2cc, None),
        p("UC3_I2CC", "UC3_I2CC", UnicommI2cc, None),
        p("UC4_I2CC", "UC4_I2CC", UnicommI2cc, None),
        p("UC5_I2CC", "UC5_I2CC", UnicommI2cc, None),
        p("UC0_I2CT", "UC0_I2CT", UnicommI2ct, None),
        p("UC1_I2CT", "UC1_I2CT", UnicommI2ct, None),
        p("UC2_I2CT", "UC2_I2CT", UnicommI2ct, None),
        p("UC3_I2CT", "UC3_I2CT", UnicommI2ct, None),
        p("UC4_I2CT", "UC4_I2CT", UnicommI2ct, None),
        p("UC5_I2CT", "UC5_I2CT", UnicommI2ct, None),
        // UC2 and UC5 have no SPI.
        p("UC0_SPI", "UC0_SPI", UnicommSpi, None),
        p("UC1_SPI", "UC1_SPI", UnicommSpi, None),
        p("UC3_SPI", "UC3_SPI", UnicommSpi, None),
        p("UC4_SPI", "UC4_SPI", UnicommSpi, None),
        p("UC0_UART", "UC0_UART", UnicommUart, Some("v1")),
        p("UC1_UART", "UC1_UART", UnicommUart, Some("v1")),
        p("UC2_UART", "UC2_UART", UnicommUart, Some("v1")),
        p("UC3_UART", "UC3_UART", UnicommUart, Some("v1")),
        p("UC4_UART", "UC4_UART", UnicommUart, Some("v1")),
        p("UC5_UART", "UC5_UART", UnicommUart, Some("v1")),
        p("WWDT0", "WWDT0", Wwdt, Some("v1")),
    ]
};

/// Names in `hw_ints.h` which differ from the peripheral they belong to.
///
/// The ports are lettered like the pins (GPIO0 is port A), and the HAL names a timer's or the DMA's
/// interrupt after the peripheral.
const IRQ_RENAMES: &[(&str, &str)] = &[
    ("GPIO0", "GPIOA"),
    ("GPIO1", "GPIOB"),
    ("GPIO2", "GPIOC"),
    ("GPIO3", "GPIOD"),
    ("DMA0", "DMA"),
    ("TIMG4_0", "TIMG4"),
    ("TIMG12_0", "TIMG12"),
    ("Debug", "DEBUGSS"),
    ("Flash", "FLASH"),
];

/// SysConfig package name, the package designator used in the chip name, and a display name.
///
/// LQFP64 and LQFP100 come in two pinouts, pin compatible with the STM32G4 (`G`) and STM32H5 (`H`)
/// respectively. The pinout letter goes in front of the designator, as in the device marking
/// (`AM13E230 19GTPM`).
const PACKAGES: &[(&str, &str, &str)] = &[
    ("LQFP128", "PDT", "TQFP-128"),
    ("LQFP100_G", "GPZ", "LQFP-100"),
    ("LQFP100_H", "HPZ", "LQFP-100"),
    ("LQFP80", "PN", "LQFP-80"),
    ("LQFP64_G", "GPM", "LQFP-64"),
    ("LQFP64_H", "HPM", "LQFP-64"),
    ("LQFP48", "PT", "LQFP-48"),
    ("QFN48", "RGZ", "VQFN-48"),
];

pub fn generate_family(family: &PartFamily, data_sources: &Path) -> anyhow::Result<()> {
    let name = &family.family;
    let hw = data_sources
        .join("am13e2x-sdk")
        .join("source")
        .join("device")
        .join(name)
        .join("include")
        .join("hw");

    let header = Header::read_files(name, &[hw.join("hw_memmap.h"), hw.join("hw_ints.h")])?;
    let pinmap = PinMap::read(&hw.join("hw_pinmap.h"))?;
    let sysconfig = read_sysconfig(data_sources, name)?;

    let descs = match name.as_str() {
        "am13e230x" => AM13E230X,
        _ => bail!("{name}: no peripheral list"),
    };

    let peripherals = generate_peripherals(name, descs, &header, &pinmap)?;
    let interrupts = generate_interrupts(name, &header)?;
    let packages = generate_packages(name, &sysconfig)?;

    let iomux = pinmap
        .pincm
        .iter()
        // The metadata numbers PINCM from 1, as SysConfig does for the MSPM0 (PINCM1 is the first
        // register there). The AM13 starts at PINCM0, so it is offset here and the IOMUX block
        // (`iomux_am13e230x`) places the array at 0 to match.
        .map(|(pin, &index)| (pin.clone(), index + 1))
        .collect::<BTreeMap<_, _>>();

    // DMA0 to DMA5 are FULL channels, DMA6 to DMA11 BASIC.
    let dma_channels = (0..12)
        .map(|n| (n, DmaChannel { full: n < 6 }))
        .collect::<BTreeMap<_, _>>();

    for part_number in family.part_numbers.iter() {
        let packages = packages
            .iter()
            .filter(|package| part_number.packages.contains(&package.package))
            .cloned()
            .map(|mut package| {
                package.chip = format!("{}{}", part_number.name, package.package.to_lowercase());
                package
            })
            .collect::<Vec<_>>();

        if packages.len() != part_number.packages.len() {
            bail!(
                "{}: a package listed in parts.yaml is not in the SysConfig metadata",
                part_number.name
            );
        }

        let chip = Chip {
            name: part_number.name.clone(),
            family: name.clone(),
            datasheet_url: family.datasheet_url.clone(),
            reference_manual_url: family.reference_manual_url.clone(),
            errata_url: family.errata_url.clone(),
            memory: part_number.memory.iter().map(convert_memory).collect(),
            packages,
            iomux: iomux.clone(),
            peripherals: peripherals.clone(),
            interrupts: interrupts.clone(),
            dma_channels: dma_channels.clone(),
            // The MSPM0 ADC fields do not apply: the AM13 ADC is a different peripheral, and has
            // no register block yet. These are the values of an MSPM0 ADC with internal and VDD
            // references only, so consumers which expect them still accept the chip.
            adc_memctl: 0,
            adc_vrsel: adc_vrsel_mapping(&family.adc_vrsel)?,
            nvic_priority_bits: Some(4),
        };

        for err in verify::verify(&chip, &part_number.name) {
            eprintln!("{err}");
        }

        let data = serde_json::to_string_pretty(&chip)
            .context(format!("Serializing chip {}", part_number.name))?;

        fs::write(format!("./build/data/{}.json", part_number.name), data)
            .context(format!("Error writing data for {}", part_number.name))?;
    }

    Ok(())
}

/// The contents of `hw_pinmap.h`.
struct PinMap {
    /// PINCM index of each pin, from 0.
    pincm: BTreeMap<String, u32>,

    /// Pin functions of each pin: function name and PF value.
    functions: BTreeMap<String, Vec<(String, u8)>>,
}

impl PinMap {
    fn read(path: &Path) -> anyhow::Result<Self> {
        /// `IOMUX_PINCM_PA26 = 26,` and `IOMUX_PINCM_PC16_X1 = 80,`
        static PINCM: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(r"(?m)^\s+IOMUX_PINCM_(?<pad>P[A-Z]\d+(?:_X\d)?)\s+=\s+(?<index>\d+),")
                .unwrap()
        });

        /// `#define IOMUX_PA26_UC0_TX_SDA_PICO 9U`
        static FUNCTION: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(
                r"(?m)^#define\s+IOMUX_(?<pad>P[A-Z]\d+(?:_X\d)?)_(?<function>\w+?)\s+(?<pf>\d+)U",
            )
            .unwrap()
        });

        let content = fs::read_to_string(path).context(format!("reading {}", path.display()))?;

        let mut pincm = BTreeMap::new();
        for c in PINCM.captures_iter(&content) {
            pincm.insert(pin_name(&c["pad"]), c["index"].parse()?);
        }

        let mut functions = BTreeMap::<String, Vec<(String, u8)>>::new();
        for c in FUNCTION.captures_iter(&content) {
            functions
                .entry(pin_name(&c["pad"]))
                .or_default()
                .push((c["function"].to_string(), c["pf"].parse()?));
        }

        if pincm.is_empty() || functions.is_empty() {
            bail!("{}: found no pins", path.display());
        }

        Ok(Self { pincm, functions })
    }
}

/// The GPIO name of a pad: `PC16_X1` is `PC16`, the crystal input being an analog function of it.
fn pin_name(pad: &str) -> String {
    pad.split_once("_X").map_or(pad, |(pin, _)| pin).to_string()
}

/// The peripheral and signals a pin function selects, or `None` for a function the metadata does
/// not describe.
///
/// UNICOMM functions name the signal of every mode the instance can run in (`UC0_TX_SDA_PICO` is
/// TX in UART mode, SDA in I2C mode and PICO in SPI mode); each becomes a signal of its own with
/// the same PF, as the MSPM0 metadata does for its UNICOMM parts.
fn classify(function: &str) -> anyhow::Result<Option<(String, Vec<String>)>> {
    static RULES: LazyLock<Vec<Regex>> = LazyLock::new(|| {
        [
            r"^GPIO\d+$",
            r"^(?<peri>UC\d)_(?<signals>.+)$",
            r"^(?<peri>TIMG\d+)_\d+_(?<signals>CCP\d)$",
            r"^(?<peri>MCPWM\d)_(?<pwm>\d[AB])$",
            r"^(?<peri>MCAN\d)_(?<signals>TX|RX)$",
            r"^(?<peri>EPI\d)_(?<signals>S\d+)$",
            r"^OUTPUTXBAR(?<out>\d+)$",
            r"^SYSCTL_(?<sysctl>XCLKOUT|FCC_IN|HFCLKIN)$",
            r"^DEBUG_(?:TMS_SWDIO|JTCK_SWCLK|JTDI|JTDO_SWO)$",
            r"^TRACE_(?:CLK|DATA\d)$",
        ]
        .iter()
        .map(|r| Regex::new(r).unwrap())
        .collect()
    });

    let Some(c) = RULES.iter().find_map(|r| r.captures(function)) else {
        bail!("unknown pin function {function}");
    };

    let result = if let Some(peri) = c.name("peri") {
        let signals = if let Some(pwm) = c.name("pwm") {
            vec![format!("PWM{}", pwm.as_str())]
        } else {
            c["signals"].split('_').map(String::from).collect()
        };
        Some((peri.as_str().to_string(), signals))
    } else if let Some(out) = c.name("out") {
        Some((
            "OUTPUTXBAR".to_string(),
            vec![format!("OUT{}", out.as_str())],
        ))
    } else if let Some(sysctl) = c.name("sysctl") {
        let signal = match sysctl.as_str() {
            "XCLKOUT" => "CLK_OUT",
            other => other,
        };
        Some(("SYSCTL".to_string(), vec![signal.to_string()]))
    } else if function == "DEBUG_TMS_SWDIO" {
        Some(("DEBUGSS".to_string(), vec!["SWDIO".to_string()]))
    } else if function == "DEBUG_JTCK_SWCLK" {
        Some(("DEBUGSS".to_string(), vec!["SWCLK".to_string()]))
    } else {
        // GPIO (added per port), the other JTAG pins and the trace port.
        None
    };

    Ok(result)
}

fn generate_peripherals(
    chip_name: &str,
    descs: &[Desc],
    header: &Header,
    pinmap: &PinMap,
) -> anyhow::Result<BTreeMap<String, Peripheral>> {
    let mut peripherals = BTreeMap::new();

    for desc in descs {
        let address = header
            .peripheral_addresses
            .get(desc.base)
            .copied()
            .context(format!("{chip_name}: no address for {}", desc.base))?;

        peripherals.insert(
            desc.name.to_string(),
            Peripheral {
                name: desc.name.to_string(),
                ty: desc.ty,
                version: desc.version.map(String::from),
                address: Some(address),
                // TODO: The power domains of the AM13 peripherals are not transcribed yet. The HAL
                // does not support low power modes on these parts.
                power_domain: PowerDomain::Pd1,
                pins: vec![],
                sys_fentries: None,
            },
        );
    }

    // One GPIO peripheral per port, holding the port's pins. GPIO is always PF 1.
    for pin in pinmap.pincm.keys() {
        let port = &pin[1..2];
        let index = (port.as_bytes()[0] - b'A') as usize;
        let base = format!("GPIO{index}");
        let address = header
            .peripheral_addresses
            .get(&base)
            .copied()
            .context(format!("{chip_name}: no address for {base}"))?;

        let gpio = peripherals
            .entry(format!("GPIO{port}"))
            .or_insert_with_key(|name| Peripheral {
                name: name.clone(),
                ty: PeripheralType::Gpio,
                version: Some("v1".to_string()),
                address: Some(address),
                power_domain: PowerDomain::Pd1,
                pins: vec![],
                sys_fentries: None,
            });

        gpio.pins.push(PeripheralPin {
            pin: pin.clone(),
            signal: pin.clone(),
            pf: Some(1),
        });
    }

    for (pin, functions) in pinmap.functions.iter() {
        for (function, pf) in functions {
            let Some((peri, signals)) =
                classify(function).context(format!("{chip_name}: {pin}"))?
            else {
                continue;
            };

            let peripheral = peripherals.get_mut(&peri).context(format!(
                "{chip_name}: pin function {function} of {pin} names unknown peripheral {peri}"
            ))?;

            for signal in signals {
                peripheral.pins.push(PeripheralPin {
                    pin: pin.clone(),
                    signal,
                    pf: Some(*pf),
                });
            }
        }
    }

    for peripheral in peripherals.values_mut() {
        peripheral.pins.sort_by(|a, b| {
            a.signal
                .cmp(&b.signal)
                .then(a.pf.cmp(&b.pf))
                .then(a.pin.cmp(&b.pin))
        });
        peripheral.pins.dedup();
    }

    Ok(peripherals)
}

fn generate_interrupts(
    chip_name: &str,
    header: &Header,
) -> anyhow::Result<BTreeMap<i32, Interrupt>> {
    let mut interrupts = BTreeMap::new();

    for (&num, entries) in header.irq_numbers.iter() {
        let [entry] = entries.as_slice() else {
            bail!("{chip_name}: interrupt {num} is shared by {entries:?}");
        };

        let name = IRQ_RENAMES
            .iter()
            .find(|(from, _)| from == entry)
            .map_or(entry.as_str(), |(_, to)| to);

        interrupts.insert(
            num,
            Interrupt {
                name: name.to_string(),
                num,
                group: BTreeMap::new(),
            },
        );
    }

    let names = interrupts
        .values()
        .map(|i| &i.name)
        .collect::<BTreeSet<_>>();
    if names.len() != interrupts.len() {
        bail!("{chip_name}: interrupt names are not unique");
    }

    Ok(interrupts)
}

/// The parts of the SysConfig device metadata used here.
#[derive(Deserialize)]
struct SysconfigFile {
    #[serde(rename = "devicePins")]
    device_pins: BTreeMap<String, DevicePin>,
    packages: BTreeMap<String, SysconfigPackage>,
}

#[derive(Deserialize)]
struct DevicePin {
    name: String,
}

#[derive(Deserialize)]
struct SysconfigPackage {
    name: String,
    #[serde(rename = "packagePin")]
    package_pins: Vec<SysconfigPackagePin>,
}

#[derive(Deserialize)]
struct SysconfigPackagePin {
    #[serde(rename = "devicePinID")]
    device_pin_id: String,
    ball: String,
}

fn read_sysconfig(data_sources: &Path, family: &str) -> anyhow::Result<SysconfigFile> {
    // SysConfig spells the family AM13E230x.
    let dir = format!("{}x", family[..family.len() - 1].to_uppercase());
    let path = data_sources
        .join("sysconfig")
        .join(&dir)
        .join(format!("{dir}.json"));
    let content = fs::read_to_string(&path).context(format!("reading {}", path.display()))?;
    serde_json::from_str(&content).context(format!("parsing {}", path.display()))
}

fn generate_packages(chip_name: &str, sysconfig: &SysconfigFile) -> anyhow::Result<Vec<Package>> {
    let mut packages = Vec::new();

    for package in sysconfig.packages.values() {
        let Some(&(_, designator, display)) = PACKAGES.iter().find(|(n, ..)| *n == package.name)
        else {
            // SysConfig also lists packages of the AM13E232x, which share its metadata.
            continue;
        };

        let mut pins = Vec::with_capacity(package.package_pins.len());
        for pin in package.package_pins.iter() {
            let device_pin = sysconfig
                .device_pins
                .get(&pin.device_pin_id)
                .context(format!(
                    "{chip_name}: {} names unknown device pin {}",
                    package.name, pin.device_pin_id
                ))?;

            // `PC16_X1` carries both the GPIO and the crystal pin.
            let signals = match device_pin.name.split_once("_X") {
                Some((gpio, x)) if gpio.starts_with('P') => {
                    vec![gpio.to_string(), format!("X{x}")]
                }
                _ => vec![device_pin.name.clone()],
            };

            pins.push(PackagePin {
                position: pin.ball.clone(),
                signals,
            });
        }

        pins.sort_by(|a, b| {
            let a_num = a.position.parse::<u32>();
            let b_num = b.position.parse::<u32>();
            match (a_num, b_num) {
                (Ok(a), Ok(b)) => a.cmp(&b),
                _ => a.position.cmp(&b.position),
            }
        });

        packages.push(Package {
            name: display.to_string(),
            chip: chip_name.to_string(),
            package: designator.to_string(),
            pins,
        });
    }

    Ok(packages)
}
