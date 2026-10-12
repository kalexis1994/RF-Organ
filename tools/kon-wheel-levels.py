#!/usr/bin/env python3
"""Reduces Kon Zissis's tonewheel generator data to what the laboratory checks.

Reads the spreadsheet published at
https://hammond-leslie.info/KonsTGandTaperingData/ (KonsData-20171011.xls) and
writes three files into OUTPUT_DIRECTORY:

busbar-levels.csv
    Each wheel connected straight to a preset-panel busbar and read at the
    preamplifier's output, as the B-3/C-3 service manual's Table V prescribes:
    that table and six consoles. What reaches a busbar from the wheel alone,
    its filter and its loading included.

manual-levels.csv
    Each wheel read through one key and drawbar at the preamplifier's output,
    as Hammond's chart "Approximate output voltage readings of organ generator
    frequencies 1 to 91" lays out for consoles with manual tapering: that
    chart and six C-3 and RT-3 consoles at full brightness. What a player
    hears of the wheel through the path it was read on, that key's tapering
    resistance included.

    Both give, per wheel, the median, lowest and highest of their curves, each
    curve in dB against its own mean over the wheels it covers.

key-tapering.csv
    For every key and drawbar of a manual, the wheel its contact reaches and
    the median of the tapering resistance wire, in ohms, across five measured
    manuals: two of a 1965 C-3, a 1966 C-3, and two of a 1960 RT-3.

    python tools/kon-wheel-levels.py KonsData-20171011.xls crates/rf-organ-lab/data

Needs the xlrd package (2.x reads .xls).
"""

import math
import os
import statistics
import sys

import xlrd

WHEELS = 91
KEYS = 61
FOOTAGES = ["16'", "5 1/3'", "8'", "4'", "2 2/3'", "2'", "1 3/5'", "1 1/3'", "1'"]

BUSBAR_SHEET = "GG + Busbar  TG output levels"
MANUAL_SHEET = "GG+dbrs&keys TG output levels"

# Each curve by a phrase its header begins with, so that a re-ordered
# workbook still selects the same organs. Where an organ was read more than
# once, the stock reading is the one taken.
BUSBAR_CURVES = [
    ("Hammond B-3 service manual, Table V", "Hammond B3 Service manual ''TABLE V"),
    ("Chris's mid-50s B-2, recapped", "Chris's recapped mid 50's B2"),
    ("Ray's 1959 C-3", "Ray's 1959 C3"),
    ("Kon's 1962 C-3, stock bass wheels", "Kon's 1962 C3. April 2007 TG curve. TG notes 1 - 12 are with the stock"),
    ("Church 1960 RT-3, stock wax capacitors", "Church 1960 RT3 with stock aged wax"),
    ("Bernard's 1973 C-3", "Bernard's 1973 C3 with the red mylar"),
    ("Jeffrey Dairiki's 1961 C-3", "Jeffrey Dairiki's 1961 C3"),
]
MANUAL_CURVES = [
    ("Hammond chart 2, consoles with manual tapering", "OLD HAMMOND GRAPH CHART TITLED \"APPROXIMATE OUTPUT VOLTAGE READINGS OF ORGAN GENERATOR FREQUENCIES 1 TO 91''. CHART 2"),
    ("Ray's 1959 C-3, full brightness", "Ray's 1959 C3 S/n 80775. TG notes 49-55 recapped with .22 mf capacitors. TG notes 55-91 recapped with .1 mf capacitors. Measured with Tone Control set at full brightness"),
    ("Kon's 1962 C-3, full brightness", "Kon's 1962 C3. April 2007 TG curve. TG notes 13 to 91 through the manuals with bass foldback.Gain trimmer capacitor at 1 1/2 turns. Tone Control set at full brightness"),
    ("Kon's 1965 C-3, January 2008", "Kon's red mylar capped 1965 C3. January 4 2008 TG recalibration. Gain trimmer cap set at 1 1/2 turns. Tone Control set to full brightness"),
    ("Kon's 1965 C-3, August 2008", "Kon's red mylar capped 1965 C3. 18 August 2008 TG recalibration. Gain trimmer cap set at 1 1/2 turns. Tone Control set to full brightness"),
    ("Phil's 1965 C-3, full brightness", "Phil's 1965 C3 With red mylar capacitors. Tone Control at full brightness"),
    ("Church 1960 RT-3, full brightness", "Church 1960 RT3 with aged wax capacitors. Tone Control at full brightness"),
]
TAPERING_CHARTS = [
    "Kon's 65 C3 Key Taper Chart UM ",
    "Kon's 65 C3 Key Taper Chart LM",
    "Peter's 66 C3 KeyTapering Chart",
    "Nigel's RT3 Key Taper Chart UM",
    "Nigel's RT3 Key Taper Chart LM",
]
SOURCE = "Kon Zissis's tonewheel generator data (hammond-leslie.info, KonsData-20171011.xls)"


def header(sheet, row, column):
    return " ".join(str(sheet.cell_value(row, column)).split())


def column_for(sheet, row, label, prefix):
    for c in range(sheet.ncols):
        if header(sheet, row, c).startswith(prefix):
            return c
    sys.exit(f"no column for {label}")


def read_curve(sheet, rows, column, wheels, label):
    """A curve in dB against its own mean over `wheels`."""
    levels = {}
    for wheel in wheels:
        cell = sheet.cell(rows[wheel], column)
        if cell.ctype != xlrd.XL_CELL_NUMBER or cell.value <= 0:
            sys.exit(f"{label} has no reading for wheel {wheel}")
        levels[wheel] = 20.0 * math.log10(float(cell.value))
    mean = sum(levels.values()) / len(levels)
    return {wheel: level - mean for wheel, level in levels.items()}


def write_levels(path, description, curves, wheels):
    with open(path, "w", encoding="utf-8", newline="\n") as handle:
        handle.write(f"# {description}\n# Written by tools/kon-wheel-levels.py from {SOURCE}.\n# Curves:\n")
        for label, _ in curves[0]:
            handle.write(f"#   {label}\n")
        handle.write("wheel,median_db,lowest_db,highest_db\n")
        for wheel in wheels:
            column = [curve[wheel] for _, curve in curves[1]]
            handle.write(
                f"{wheel},{statistics.median(column):.3f},{min(column):.3f},{max(column):.3f}\n"
            )


def tapering_chart(book, name):
    sheet = book.sheet_by_name(name)
    # The charts letter the 1 1/3' drawbar as 1 2/3'; its wheels are the 1 1/3' ones.
    columns = [
        c for c in range(sheet.ncols) if header(sheet, 1, c).replace("1 2/3'", "1 1/3'") in FOOTAGES
    ]
    if len(columns) != len(FOOTAGES):
        sys.exit(f"{name} does not have the nine drawbars")
    table = {}
    key = 0
    for r in range(2, sheet.nrows):
        if sheet.cell_type(r, columns[0]) != xlrd.XL_CELL_NUMBER:
            continue
        key += 1
        for bus, c in enumerate(columns):
            table[(key, bus)] = (int(sheet.cell_value(r, c)), float(sheet.cell_value(r, c + 1)))
    if key != KEYS:
        sys.exit(f"{name} lists {key} keys")
    return table


def main(source, destination):
    book = xlrd.open_workbook(source, on_demand=True)

    busbar = book.sheet_by_name(BUSBAR_SHEET)
    busbar_rows = {}
    for r in range(2, busbar.nrows):
        if busbar.cell_type(r, 0) == xlrd.XL_CELL_NUMBER:
            busbar_rows.setdefault(int(busbar.cell_value(r, 0)), r)
    all_wheels = range(1, WHEELS + 1)
    busbar_curves = [
        (label, read_curve(busbar, busbar_rows, column_for(busbar, 1, label, prefix), all_wheels, label))
        for label, prefix in BUSBAR_CURVES
    ]
    write_levels(
        os.path.join(destination, "busbar-levels.csv"),
        "Each wheel straight onto a busbar, read at the preamplifier's output (Table V protocol).",
        (BUSBAR_CURVES, busbar_curves),
        all_wheels,
    )

    manual = book.sheet_by_name(MANUAL_SHEET)
    manual_rows = {}
    for r in range(7, manual.nrows):
        if manual.cell_type(r, 6) == xlrd.XL_CELL_NUMBER:
            manual_rows.setdefault(int(manual.cell_value(r, 6)), r)
    manual_wheels = range(13, WHEELS + 1)
    manual_curves = []
    for label, prefix in MANUAL_CURVES:
        column = column_for(manual, 5, label, prefix)
        # Hammond's chart lays its voltages three columns after its title.
        if column == 4:
            column = 7
        manual_curves.append((label, read_curve(manual, manual_rows, column, manual_wheels, label)))
    write_levels(
        os.path.join(destination, "manual-levels.csv"),
        "Each wheel through one key and drawbar, read at the preamplifier's output (Hammond chart 2 protocol).",
        (MANUAL_CURVES, manual_curves),
        manual_wheels,
    )

    charts = [tapering_chart(book, name) for name in TAPERING_CHARTS]
    with open(os.path.join(destination, "key-tapering.csv"), "w", encoding="utf-8", newline="\n") as handle:
        handle.write(
            "# Manual key tapering: the wheel each key's contact reaches for each drawbar,\n"
            "# and the median tapering resistance wire in ohms across the charts below.\n"
            f"# Written by tools/kon-wheel-levels.py from {SOURCE}.\n# Charts:\n"
        )
        for name in TAPERING_CHARTS:
            handle.write(f"#   {name.strip()}\n")
        handle.write("key,drawbar,wheel,charts_agreeing,ohms\n")
        for key in range(1, KEYS + 1):
            for bus in range(len(FOOTAGES)):
                wheels = [chart[(key, bus)][0] for chart in charts]
                wheel = statistics.mode(wheels)
                ohms = statistics.median(chart[(key, bus)][1] for chart in charts)
                agreeing = wheels.count(wheel)
                handle.write(f"{key},{bus + 1},{wheel},{agreeing},{ohms:g}\n")


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit("usage: kon-wheel-levels.py KonsData.xls OUTPUT_DIRECTORY")
    main(sys.argv[1], sys.argv[2])
