# mspm0-data

`mspm0-data` aims to produce clean machine-readable data about MSPM0 microcontroller families, including:

- ✔️ Base chip information
  - Packages
  - 🚧 Flash, RAM size
- ✔️ Peripheral addresses and interrupts
- ✔️ Interrupts
- ✔️ GPIO peripheral function (PF) mappings
- 🚧 Register blocks for all peripherals
- 🚧 DMA mappings
- ✔️ Per package pinouts
- 🚧 Links to applicable technical reference manual and datasheet PDFs.

✔️ = done, 🚧 = work in progress, ❌ = to do

The generated JSON files are [available here in the mspm0-data-generated](https://github.com/mspm0-rs/mspm0-data-generated/tree/master/data) repo.

# mspm0-metapac

The generated PAC is [available here in the mspm0-data-generated](https://github.com/mspm0-rs/mspm0-data-generated/tree/master/mspm0-metapac) repo.

# AM13E2x (AM13E230x, AM13E232x)

This fork also generates the TI **AM13E2x** (Arm Cortex-M33) parts:

* AM13E230x: `am13e23019`, `am13e23018` and `am13e23017`.
* AM13E232x: `am13e23219`, `am13e23218` and `am13e232n9`, which add an EtherCAT slave controller,
  ePWM, SDFM, FSI and a second MCAN.

They are built from MSPM0 peripherals (GPIO, IOMUX, GPTIMER, DMA, UNICOMM, WWDT), which share the
MSPM0 register blocks or close variants of them, next to C2000 style control peripherals. Their
sources are laid out differently from the MSPM0 ones, so they go through
[`mspm0-data-gen/src/am13.rs`](./mspm0-data-gen/src/am13.rs) instead of the MSPM0 path:

* Peripheral addresses and interrupts: `hw_memmap.h` and `hw_ints.h` of the AM13E2x SDKs (the
  public [AM13E2x SDK] for the AM13E230x, SDK 26.02.00.03 for the AM13E232x).
* Pins, PINCM indices and pin functions: `hw_pinmap.h` of the SDK. The SysConfig metadata of these
  parts has neither the PINCM nor the PF values.
* NVIC priority bits: `soc.h` of the SDK (4 on the AM13E230x, 2 on the AM13E232x).
* Packages and pinouts: SysConfig (`sysconfig/AM13E230x`, `sysconfig/AM13E232x`).
* Registers: TI ships no SVD for these families. `tools/targetdb2svd.py` in mspm0-data-sources
  builds `svd/AM13E230X.svd` and `svd/AM13E232X.svd` from the register descriptions in the CCS
  targetdb, adding the KEY fields the targetdb omits from the SDK headers.

Register blocks (the same on both families unless noted):

| Peripheral | Block | Source |
|---|---|---|
| GPIO, TIMG4/TIMG12, WWDT0 | `gpio_v1`, `tim_v1`, `wwdt_v1` | The MSPM0 blocks. Checked field by field and enum by enum against both AM13 SVDs. |
| DMA | `dma_v2` | `dma_v1` with 12 channels, a 7-bit trigger select and 128-bit transfers ([`transforms/am13.py`](./transforms/am13.py)). |
| IOMUX | `iomux_am13` | `iomux_v1` with PINCM0 at offset 0 (117 entries; the AM13E230x uses the first 108). |
| UCx | `unicomm_v1` | The UNICOMM wrapper: power, reset and mode selection. |
| UCx_UART | `unicommuart_v1` | `uart_v1` moved down by 0x1000, with the AM13 status bits, FIFO levels and interrupt indexes. |
| UCx_SPI | `unicommspi_v1` | The AM13 SVDs, through [`transforms/UNICOMMSPI.yaml`](./transforms/UNICOMMSPI.yaml). UC2 and UC5 have no SPI. |
| UCx_I2CC | `unicommi2cc_v1` | The I2C controller registers, from the AM13 SVDs through [`transforms/UNICOMMI2CC.yaml`](./transforms/UNICOMMI2CC.yaml). |
| ADC0..2 | `adc_am13` | The AM13 SVDs, through [`transforms/ADC_AM13.yaml`](./transforms/ADC_AM13.yaml). A SOC and sequencer based ADC, not the MSPM0 ADC12. |
| ADC0RESULT..ADC2RESULT | `adcresult_am13` | The result registers of each ADC, a separate block ([`transforms/ADCRESULT_AM13.yaml`](./transforms/ADCRESULT_AM13.yaml)). |
| SYSCTL | `sysctl_am13e230x`, `sysctl_am13e232x` | The AM13 SVDs, through [`transforms/SYSCTL_AM13E230X.yaml`](./transforms/SYSCTL_AM13E230X.yaml) and [`transforms/SYSCTL_AM13E232X.yaml`](./transforms/SYSCTL_AM13E232X.yaml). The AM13E232x one adds the EtherCAT and SDFM controls at the same offsets. |

Differences from the MSPM0 metadata:

* GPIO ports are named `GPIOA` to `GPIOD` like the pins (the SDK calls them `GPIO0` to `GPIO3`),
  and each port has an interrupt of its own. There are no interrupt groups.
* PINCM numbers are given from 1, like SysConfig gives them for the MSPM0; the AM13 registers start
  at PINCM0, which `iomux_am13` accounts for.
* A UNICOMM instance `UCx` is the wrapper. The registers of each mode are separate peripherals,
  `UCx_UART` and so on.
* Flash is two banks at 0x0 and 0x40000. Only the 512 KB parts fill both, so the smaller parts have
  a separate `FLASH_BANK1` region.
* Power domains are not transcribed yet.
* The ADC inputs come from the SysConfig metadata (`hw_pinmap.h` does not list analog functions):
  the signal of an ADC pin is its channel number (CHSEL), with PF 0, as for the MSPM0 ADC. The
  results are in the `ADCxRESULT` peripherals.
* Chip names carry the package designator. On the AM13E230x the pinout variant of the LQFP-64 and
  LQFP-100 packages (STM32G4 or STM32H5 compatible) goes in front: `am13e23019gpm` is the LQFP-64 G
  part on the LP-AM13E230 LaunchPad. The AM13E232x datasheet is not public yet, so its packages keep
  the SysConfig names (`am13e23219lqfp64g`), except the 128-pin TQFP: `am13e232n9pdt` is the part on
  the LP-AM13E232 LaunchPad (`AM13E232N91TPDTR`).

The MSPM0 output of this fork is identical to upstream's.

[AM13E2x SDK]: https://github.com/TexasInstruments/am13e2x_sdk

# Data sources

These are the data sources currently used.

* SysConfig metadata from Code Composer Studio
  * Packages and package pinouts
  * Mapping from GPIO pin to IOMUX::PINCM register.
  * Peripheral PF (pin function) mappings.
  * Peripheral pin names.
* mspm0-sdk header files
  * Interrupt number, name
  * Peripheral addresses
* mspm0 SVDs: register blocks
* Manually entered
  * IIDX values for interrupts within a `INT_GROUP`

# Adding a new chip

1. Update the data sources to include the new chip. You will need to get the SVD and sysconfig metadata.
2. Add the new chip family and part numbers to [`parts.yaml`](./data/parts.yaml)
3. If needed, add any chip specific register blocks like `sysctl`.
4. Check the peripheral mapping in [`perimap.rs`](./mspm0-data-gen/src/perimap.rs) to use the correct peripherals.

# Adding support for a new peripheral

This will help you add support for a new peripheral to all MSPM0 families. (Please take the time to add it for all families, even if you personally
are only interested in one. It's easier than it looks, and doing all families at once is significantly less work than adding one now then having to revisit everything later when adding more. It also helps massively in catching mistakes and inconsistencies in the source SVDs.)

- Install chiptool with `./d install-chiptool`
- Download MSPM0 data sources with `./d download-all`
- Run `./d extract-all CANFD0`. This'll output a bunch of yamls in `tmp/CANFD0`. NOTE sometimes peripherals have a number sometimes not (`CANFD0` vs `CANFD`). You might want to try both and merge the outputted YAMLs into a single directory.
- Diff them between themselves, to identify differences. The differences can either be:
  - 1: Legitimate differences between families, because there are different CANFD versions. For example, added registers/fields.
  - 2: SVD inconsistencies, like different register names for the same register
  - 3: SVD mistakes (yes, there are some)
  - 4: Missing stuff in SVDs, usually enums or doc descriptions.
- Identify how many actually-different (incompatible) versions of the peripheral exist, as we must _not_ merge them. Name them v1, v2.. (if possible, by order of chip release date
- For each version, pick the "best" YAML (the one that has less enums/docs missing), place them in `data/registers/canfd_vX.yaml`
- Cleanup the register yamls (see below).
- Minimize the diff between each pair of versions. For example between `canfd_v1.yaml` and `canfd_v2.yaml`. If one is missing enums or descriptions, copy it from another.
- Add entries to [`perimap`](./mspm0-data-gen/src/perimap.rs), see below.
- Add corresponding `PeripheralType` to [`GENERATE_PERIPHERALS`](./mspm0-metapac-gen/src/peripheral.rs).
- Rebuild (`./d gen && ./d build-metapac`), then:
  - Check `mspm0-metapac/src/chips/<chip>/pac.rs` has the right `#[path = "../../peripherals/canfd_v1.rs"]` paths.
  - Ensure a successful build of the affected pac. e.g.

    ```
    cd build/mspm0-metapac
    cargo build --features mspm0g3507pm
    ```

Please separate manual changes and changes resulting from regen in separate commits. It helps tremendously with review and rebasing/merging.

## Register cleanup

SVDs have some widespread annoyances that should be fixed when adding register YAMLs to this repo. Check out `chiptool` transforms, they can help in speeding up the cleanups.

- Remove "useless prefixes". For example if all regs in the `RNG` peripheral are named `RNG_FOO`, `RNG_BAR`, the `RNG_` peripheral is not conveying any useful information at all, and must go.
- Remove "useless enums". Useless enums is one of the biggest cause of slow compilation times in STM32 PACs.
  - 0=disabled, 1=enabled. Common in `xxEN` and `xxIE` fields. If a field says "enable foo" and is one bit, it's obvious "true" means enabled and "false" means disabled.
  - "Write 0/1 to clear" enums, common in `xxIF` fields.
  - Check out the `DeleteEnums` chiptool transforms.
- Convert repeated registers or fields (like `FOO0 FOO1, FOO2, FOO3`) to arrays `FOO[n]`.
  - Check out the `MakeRegisterArray`, `MakeFieldArray` chiptool transforms.
- Use `chiptool fmt` on each of the register yamls.

## Peripheral mapping (perimap)

The `mspm0-data-gen` binary has a map to match peripherals to the right version in all chips, the [perimap](./mspm0-data-gen/src/perimap.rs).

When parsing a chip, for each peripheral a "key" string is constructed using this format: `FAMILY:PERIPHERAL_NAME`, where:

- `FAMILY`: chip family in lowercase, for example `mspm0g350x`
- `PERIPHERAL_NAME`: peripheral name, for example `spi`.

`perimap` entries are regexes matching on the above "key" string. First regex that matches wins. For example:

```
(".*:tim", ("tim", "v1")),
("mspm0c110x:sysctl", ("sysctl", "c110x")),
("mspm0g..0x:sysctl", ("sysctl", "g350x_g310x_g150x_g110x")),
```
