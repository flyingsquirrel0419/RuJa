#!/usr/bin/env python3
"""Generate compact Intl.Locale-info tables from pinned CLDR data."""

from __future__ import annotations

import argparse
import urllib.request
import xml.etree.ElementTree as ET
from collections import defaultdict
from pathlib import Path


CLDR_VERSION = "48.2"
CLDR_REF = "11299982335beb974c1c63c45265184e759c0f41"
RAW_ROOT = f"https://raw.githubusercontent.com/unicode-org/cldr/{CLDR_REF}/common"
DAY_NUMBER = {"mon": 1, "tue": 2, "wed": 3, "thu": 4, "fri": 5, "sat": 6, "sun": 7}
HOUR_CYCLE = {"H": "h23", "k": "h24", "h": "h12", "K": "h11"}


def load(path: str) -> bytes:
    with urllib.request.urlopen(f"{RAW_ROOT}/{path}", timeout=30) as response:
        return response.read()


def load_xml(path: str) -> ET.Element:
    return ET.fromstring(load(path))


def canonical_calendar_names() -> dict[str, str]:
    root = load_xml("bcp47/calendar.xml")
    aliases: dict[str, str] = {}
    for node in root.findall(".//type"):
        name = node.attrib["name"].lower()
        canonical = node.attrib.get("preferred", name).lower()
        aliases[name] = canonical
        for alias in node.attrib.get("alias", "").split():
            aliases[alias.lower()] = canonical
    return aliases


def calendar_preferences(root: ET.Element) -> dict[str, list[str]]:
    aliases = canonical_calendar_names()
    result: dict[str, list[str]] = {}
    for node in root.findall(".//calendarPreferenceData/calendarPreference"):
        values = []
        for value in node.attrib["ordering"].split():
            canonical = aliases.get(value.lower(), value.lower())
            if canonical not in values:
                values.append(canonical)
        for territory in node.attrib["territories"].split():
            result[territory] = values
    return result


def hour_cycles(root: ET.Element) -> dict[str, list[str]]:
    result: dict[str, list[str]] = {}
    for node in root.findall(".//timeData/hours"):
        values = []
        for symbol in (node.attrib["preferred"] + " " + node.attrib["allowed"]).split():
            cycle = HOUR_CYCLE.get(symbol[0])
            if cycle is not None and cycle not in values:
                values.append(cycle)
        for locale in node.attrib["regions"].split():
            result[locale.replace("_", "-")] = values
    return result


def week_information(root: ET.Element) -> dict[str, tuple[int, list[int]]]:
    fields: dict[str, dict[str, int]] = defaultdict(dict)
    for element in ("firstDay", "weekendStart", "weekendEnd"):
        for node in root.findall(f".//weekData/{element}"):
            if "alt" in node.attrib:
                continue
            day = DAY_NUMBER[node.attrib["day"]]
            for territory in node.attrib["territories"].split():
                fields[territory][element] = day

    defaults = fields["001"]
    result = {}
    for territory in sorted(fields):
        data = fields[territory]
        first = data.get("firstDay", defaults["firstDay"])
        start = data.get("weekendStart", defaults["weekendStart"])
        end = data.get("weekendEnd", defaults["weekendEnd"])
        weekend = []
        day = start
        while True:
            weekend.append(day)
            if day == end:
                break
            day = day % 7 + 1
        result[territory] = (first, sorted(weekend))
    return result


def script_directions() -> dict[str, str]:
    result = {}
    for raw_line in load("properties/scriptMetadata.txt").decode("utf-8").splitlines():
        line = raw_line.split("#", 1)[0].strip()
        if not line:
            continue
        fields = [field.strip() for field in line.split(";")]
        if fields[6] == "YES":
            result[fields[0]] = "rtl"
        elif fields[6] == "NO":
            result[fields[0]] = "ltr"
    return result


def canonical_time_zone_names() -> dict[str, str]:
    root = load_xml("bcp47/timezone.xml")
    aliases = {}
    for node in root.findall(".//type"):
        names = node.attrib.get("alias", "").split()
        canonical = node.attrib.get("iana") or (names[0] if names else None)
        if canonical is None or "/" not in canonical:
            continue
        aliases[canonical] = canonical
        for alias in names:
            aliases[alias] = canonical
    return aliases


def time_zones() -> dict[str, list[str]]:
    aliases = canonical_time_zone_names()
    result: dict[str, set[str]] = defaultdict(set)
    time_zone_root = load_xml("bcp47/timezone.xml")
    for node in time_zone_root.findall(".//type"):
        if node.attrib.get("deprecated") == "true":
            continue
        name = node.attrib["name"]
        names = node.attrib.get("alias", "").split()
        canonical = node.attrib.get("iana") or (names[0] if names else None)
        region = node.attrib.get("region") or (name[:2].upper() if len(name) == 5 else None)
        if (
            canonical is not None
            and "/" in canonical
            and region is not None
            and len(region) == 2
            and region.isalpha()
            and region != "ZZ"
        ):
            result[region].add(canonical)
    root = load_xml("supplemental/windowsZones.xml")
    for node in root.findall(".//mapZone"):
        territory = node.attrib["territory"]
        if territory in ("001", "ZZ"):
            continue
        for identifier in node.attrib["type"].split():
            canonical = aliases.get(identifier)
            if canonical is None:
                raise ValueError(f"no canonical IANA ID for {identifier}")
            result[territory].add(canonical)
    return {territory: sorted(values) for territory, values in result.items()}


def render_string_lists(name: str, values: dict[str, list[str]]) -> list[str]:
    lines = ["#[rustfmt::skip]\n", f"pub(super) const {name}: &[(&str, &[&str])] = &[\n"]
    for key in sorted(values):
        rendered = ", ".join(f'"{value}"' for value in values[key])
        lines.append(f'    ("{key}", &[{rendered}]),\n')
    lines.append("];\n")
    return lines


def render_week_information(values: dict[str, tuple[int, list[int]]]) -> list[str]:
    lines = [
        "#[rustfmt::skip]\n",
        "pub(super) const WEEK_INFORMATION: &[(&str, u8, &[u8])] = &[\n",
    ]
    for territory in sorted(values):
        first, weekend = values[territory]
        rendered = ", ".join(str(day) for day in weekend)
        lines.append(f'    ("{territory}", {first}, &[{rendered}]),\n')
    lines.append("];\n")
    return lines


def render_directions(values: dict[str, str]) -> list[str]:
    lines = [
        "#[rustfmt::skip]\n",
        "pub(super) const SCRIPT_DIRECTIONS: &[(&str, &str)] = &[\n",
    ]
    for script in sorted(values):
        lines.append(f'    ("{script}", "{values[script]}"),\n')
    lines.append("];\n")
    return lines


def generate() -> str:
    supplemental = load_xml("supplemental/supplementalData.xml")
    output = [
        "// @generated by tools/generate_intl_locale_info.py; do not edit.\n",
        f"// Source: Unicode CLDR {CLDR_VERSION} at {CLDR_REF}.\n\n",
    ]
    output.extend(render_string_lists("CALENDAR_PREFERENCES", calendar_preferences(supplemental)))
    output.append("\n")
    output.extend(render_string_lists("HOUR_CYCLES", hour_cycles(supplemental)))
    output.append("\n")
    output.extend(render_week_information(week_information(supplemental)))
    output.append("\n")
    output.extend(render_directions(script_directions()))
    output.append("\n")
    output.extend(render_string_lists("TIME_ZONES", time_zones()))
    return "".join(output)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--output", type=Path, default=Path("src/builtins/intl_locale_info.rs")
    )
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    formatted = generate()
    if args.check:
        if not args.output.is_file() or args.output.read_text(encoding="utf-8") != formatted:
            raise SystemExit(f"{args.output} is not generated from pinned CLDR data")
    else:
        args.output.write_text(formatted, encoding="utf-8")


if __name__ == "__main__":
    main()
