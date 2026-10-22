#!/usr/bin/env python3
"""Derive the AM13E230x register blocks which are MSPM0 IP with small changes.

The AM13E230x reuses several MSPM0 peripherals. Where the AM13 block is the MSPM0 block with a
few fields resized or removed, it is derived here from the curated MSPM0 YAML rather than
re-extracted from the AM13 SVD, so the two keep the same register, field and enum names and the
HAL can drive both with the same code.

Every difference applied below was checked against the AM13E230X SVD (built from the CCS
targetdb by `tools/targetdb2svd.py` in mspm0-data-sources) and the AM13E230x TRM.

Run from the repository root:

    python transforms/am13e230x.py
"""

import copy
from pathlib import Path

import yaml

REGISTERS = Path("data/registers")


def load(name):
    return yaml.safe_load((REGISTERS / f"{name}.yaml").read_text())


def save(name, data, header):
    text = yaml.safe_dump(data, sort_keys=False, width=1000, allow_unicode=True)
    (REGISTERS / f"{name}.yaml").write_text(header + text, newline="\n")


def item(block, name):
    return next(i for i in block["items"] if i["name"] == name)


def field(fieldset, name):
    return next(f for f in fieldset["fields"] if f["name"] == name)


def drop_items(block, *names):
    block["items"] = [i for i in block["items"] if i["name"] not in names]


def drop_fields(fieldset, *names):
    fieldset["fields"] = [f for f in fieldset["fields"] if f["name"] not in names]


def iomux():
    """IOMUX: PINCM0 is the first register, where MSPM0 starts at PINCM1 (offset 4)."""
    d = load("iomux_v1")
    pincm = item(d["block/IOMUX"], "PINCM")
    pincm["byte_offset"] = 0
    # PINCM0..PINCM107. Index 106 is NRST, which has no pin function.
    pincm["array"]["len"] = 108
    save(
        "iomux_am13e230x",
        d,
        "# AM13E230x IOMUX, derived from iomux_v1 by transforms/am13e230x.py.\n"
        "# The PINCM array starts at offset 0 (PINCM0 = PA0) instead of 4.\n",
    )


def dma():
    """DMA with 12 channels, a 7-bit trigger select and a 128-bit transfer width."""
    d = load("dma_v1")
    blk = d["block/DMA"]

    # No event fabric publisher/subscriber ports, and only the CPU interrupt event.
    drop_items(blk, "FSUB", "FPUB")
    del d["fieldset/FPORT"]
    item(blk, "INT_EVENT")["array"]["len"] = 1

    # DMA0..DMA5 are FULL channels, DMA6..DMA11 BASIC.
    item(blk, "TRIG")["array"]["len"] = 12
    item(blk, "CHAN")["array"]["len"] = 12

    field(d["fieldset/TCTL"], "TSEL")["bit_size"] = 7

    ctl = d["fieldset/CTL"]
    field(ctl, "SRCWDTH")["bit_size"] = 3
    field(ctl, "DSTWDTH")["bit_size"] = 3
    wdth = d["enum/WDTH"]
    wdth["bit_size"] = 3
    wdth["variants"].append(
        {
            "name": "LONGLONG",
            "description": "Data width is LONG-LONG-WORD (128-bit).",
            "value": 4,
        }
    )

    ints = d["fieldset/INT"]
    field(ints, "CH")["array"]["len"] = 12
    field(ints, "PREIRQCH")["array"]["len"] = 6
    drop_fields(ints, "DATAERR")

    save(
        "dma_v2",
        d,
        "# DMA with 12 channels (AM13E230x), derived from dma_v1 by transforms/am13e230x.py.\n",
    )


def unicommuart():
    """The UART of a UNICOMM instance.

    The register set is the MSPM0 UART one, 0x1000 lower, with power and reset moved to the
    UNICOMM wrapper (see unicomm_v1) and without IrDA, Manchester encoding or the FIFO enable bit
    (the FIFOs are always on).
    """
    d = load("uart_v1")
    blk = d.pop("block/UART")

    drop_items(blk, "GPRCM", "IRCTL", "CLKDIV2")
    for k in ["block/GPRCM", "fieldset/PWREN", "fieldset/RSTCTL", "fieldset/CLKCFG",
              "fieldset/GPRCM_STAT", "fieldset/IRCTL", "enum/PWREN_KEY", "enum/RESET_KEY",
              "enum/CLKCFG_KEY", "enum/IRTXCLK", "enum/IRRXPL"]:
        d.pop(k, None)

    for i in blk["items"]:
        i["byte_offset"] -= 0x1000

    # UARTclk is MCLK/2, divided by 1..64.
    d["fieldset/CLKSEL"]["fields"] = [
        {
            "name": "BUSCLK_SEL",
            "description": "Selects MCLK/2 as the clock source.",
            "bit_offset": 3,
            "bit_size": 1,
        }
    ]
    div = field(d["fieldset/CLKDIV"], "RATIO")
    div["bit_size"] = 6
    div.pop("enum", None)
    div["description"] = "Selects the divide ratio of the module clock: RATIO + 1."
    d.pop("enum/CLKDIV", None)

    drop_fields(d["fieldset/CTL0"], "MENC", "FEN")

    # The DMA trigger events only have IMASK, RIS, MIS and ISET.
    for trig in ("block/DMA_TRIG_RX", "block/DMA_TRIG_TX"):
        drop_items(d[trig], "IIDX", "ICLR")
    for k in ("fieldset/DMA_TRIG_RX_IIDX", "fieldset/DMA_TRIG_TX_IIDX",
              "enum/DMA_TRIG_RX_IIDX_STAT", "enum/DMA_TRIG_TX_IIDX_STAT"):
        d.pop(k, None)

    # No event publishing from the UART of a UNICOMM.
    drop_items(blk, "EVT_MODE")
    d.pop("fieldset/EVT_MODE", None)
    d.pop("enum/INT_CFG", None)

    def add(fieldset, *fields):
        fs = d[f"fieldset/{fieldset}"]
        for name, bit, desc in fields:
            fs["fields"].append(
                {"name": name, "description": desc, "bit_offset": bit, "bit_size": 1}
            )
        fs["fields"].sort(key=lambda f: f["bit_offset"])

    def move(fieldset, **bits):
        for name, bit in bits.items():
            field(d[f"fieldset/{fieldset}"], name)["bit_offset"] = bit
        d[f"fieldset/{fieldset}"]["fields"].sort(key=lambda f: f["bit_offset"])

    # STAT is laid out differently: the FIFO clear status bits sit next to the FIFO flags.
    move("STAT", TXFE=5, TXFF=6)
    add(
        "STAT",
        ("RXCLR", 4, "RX FIFO clear status."),
        ("TXCLR", 7, "TX FIFO clear status."),
        ("SENDIDLE", 10, "Send idle pattern status."),
    )

    add(
        "IFLS",
        ("TXCLR", 3, "Clear the TX FIFO."),
        ("RXCLR", 7, "Clear the RX FIFO."),
    )

    # The FIFO trigger levels are encoded differently from the MSPM0 UART, and differently for
    # TX and RX (hw_unicommuart.h, UNICOMMUART_IFLS_TXIFLSEL_* and _RXIFLSEL_*).
    def levels(*variants):
        return {
            "bit_size": 3,
            "variants": [
                {"name": name, "description": desc, "value": value}
                for value, name, desc in variants
            ],
        }

    d.pop("enum/IFLSSEL")
    d["enum/TXIFLSEL"] = levels(
        (1, "THREE_FOURTH", "TX FIFO <= 3/4 empty."),
        (2, "HALF", "TX FIFO <= 1/2 empty."),
        (3, "ONE_FOURTH", "TX FIFO <= 1/4 empty."),
        (4, "NOT_FULL", "TX FIFO is not full."),
        (5, "EMPTY", "TX FIFO is empty."),
        (6, "ALMOST_EMPTY", "TX FIFO holds at most one entry."),
        (7, "ALMOST_FULL", "TX FIFO has at most one free entry."),
    )
    d["enum/RXIFLSEL"] = levels(
        (1, "ONE_FOURTH", "RX FIFO >= 1/4 full."),
        (2, "HALF", "RX FIFO >= 1/2 full."),
        (3, "THREE_FOURTH", "RX FIFO >= 3/4 full."),
        (4, "NOT_EMPTY", "RX FIFO is not empty."),
        (5, "FULL", "RX FIFO is full."),
        (6, "ALMOST_FULL", "RX FIFO has at most one free entry."),
        (7, "ALMOST_EMPTY", "RX FIFO holds at most one entry."),
    )
    field(d["fieldset/IFLS"], "TXIFLSEL")["enum"] = "TXIFLSEL"
    field(d["fieldset/IFLS"], "RXIFLSEL")["enum"] = "RXIFLSEL"

    # Interrupt indexes: 14 is the address match, and there are three more sources.
    stat = d["enum/CPU_INT_IIDX_STAT"]["variants"]
    for v in stat:
        if v["value"] == 14:
            v["name"] = "ADDR_MATCH"
            v["description"] = "Address match interrupt."
    stat += [
        {"name": "DMA_PREIRQ_RX", "description": "DMA pre-IRQ on the RX channel.", "value": 19},
        {"name": "DMA_PREIRQ_TX", "description": "DMA pre-IRQ on the TX channel.", "value": 20},
        {"name": "LTFG", "description": "LIN timeout.", "value": 21},
    ]

    # RXDATA has no overrun flag (overruns are reported in RIS only) and NERR moves down.
    drop_fields(d["fieldset/RXDATA"], "OVRERR")
    move("RXDATA", NERR=11)

    # Only the digital glitch filter exists.
    drop_fields(d["fieldset/GFCTL"], "AGFEN", "AGFSEL", "CHAIN")
    d.pop("enum/AGFSEL", None)

    add("LCRH", ("SUSPEND", 26, "Suspend external communication."))
    add("LINCTL", ("EN_FRM_ERR", 7, "Enable framing error detection in LIN mode."))
    add("CPU_INT", ("LTOUT", 20, "LIN timeout."))

    blk["description"] = "UART mode registers of a UNICOMM instance."
    out = {"block/UNICOMMUART": blk}
    out.update(sorted((k, v) for k, v in d.items()))
    save(
        "unicommuart_v1",
        out,
        "# UART of a UNICOMM instance (AM13E230x), derived from uart_v1 by\n"
        "# transforms/am13e230x.py.\n",
    )


def unicomm():
    """The UNICOMM wrapper: power, reset and the selection of the active serial mode.

    The GPRCM is the standard one (taken from uart_v1, keys included; the AM13 SDK writes the
    same keys). Each mode has its own register block at a separate address, described by
    unicommuart_v1 and so on.
    """
    v = load("uart_v1")
    out = {
        "block/UNICOMM": {
            "description": "UNICOMM wrapper: power, reset and serial mode selection.",
            "items": [
                {
                    "name": "GPRCM",
                    "array": {"len": 1, "stride": 24},
                    "byte_offset": 0x800,
                    "block": "GPRCM",
                },
                {
                    "name": "IPMODE",
                    "description": "Mode Selection Register.",
                    "byte_offset": 0x1100,
                    "fieldset": "IPMODE",
                },
            ],
        },
        "block/GPRCM": v["block/GPRCM"],
        "fieldset/CLKCFG": v["fieldset/CLKCFG"],
        "fieldset/GPRCM_STAT": v["fieldset/GPRCM_STAT"],
        "fieldset/IPMODE": {
            "description": "Mode Selection Register.",
            "fields": [
                {
                    "name": "SELECT",
                    "description": "Selects which serial protocol the UNICOMM instance runs.",
                    "bit_offset": 0,
                    "bit_size": 2,
                    "enum": "SELECT",
                }
            ],
        },
        "fieldset/PWREN": v["fieldset/PWREN"],
        "fieldset/RSTCTL": v["fieldset/RSTCTL"],
        "enum/CLKCFG_KEY": v["enum/CLKCFG_KEY"],
        "enum/PWREN_KEY": v["enum/PWREN_KEY"],
        "enum/RESET_KEY": v["enum/RESET_KEY"],
        "enum/SELECT": {
            "bit_size": 2,
            "variants": [
                {"name": "UART", "description": "UART mode.", "value": 0},
                {"name": "SPI", "description": "SPI mode.", "value": 1},
                {"name": "I2C_CONTROLLER", "description": "I2C controller mode.", "value": 2},
                {"name": "I2C_TARGET", "description": "I2C target mode.", "value": 3},
            ],
        },
    }
    save(
        "unicomm_v1",
        out,
        "# UNICOMM wrapper (AM13E230x), written by transforms/am13e230x.py.\n",
    )


if __name__ == "__main__":
    iomux()
    dma()
    unicommuart()
    unicomm()
