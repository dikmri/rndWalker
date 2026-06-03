#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
import time
import urllib.parse
import urllib.request
from pathlib import Path

LANGUAGES = {
    "en": "English",
    "zh-CN": "Simplified Chinese",
    "ko": "Korean",
}

GOOGLE_TRANSLATE_URL = "https://translate.googleapis.com/translate_a/single"


def translate_text(text: str, target: str) -> str:
    if not text.strip():
        return text

    params = urllib.parse.urlencode(
        {
            "client": "gtx",
            "sl": "ja",
            "tl": target,
            "dt": "t",
            "q": text,
        }
    )
    request = urllib.request.Request(
        f"{GOOGLE_TRANSLATE_URL}?{params}",
        headers={"User-Agent": "rndWalker-docs-i18n/1.0"},
    )

    last_error: Exception | None = None
    for _ in range(3):
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                payload = json.loads(response.read().decode("utf-8"))
            return "".join(part[0] for part in payload[0] if part and part[0])
        except Exception as error:  # noqa: BLE001
            last_error = error
            time.sleep(1.0)

    raise RuntimeError(f"translation failed for {target}: {last_error}")


def translate_table_row(line: str, target: str) -> str:
    if re.fullmatch(r"\s*\|?[\s:\-|\u3000]+\|?\s*", line):
        return line

    leading = "|" if line.startswith("|") else ""
    trailing = "|" if line.endswith("|") else ""
    cells = line.strip("|").split("|")
    translated = []
    for cell in cells:
        left = cell[: len(cell) - len(cell.lstrip())]
        right = cell[len(cell.rstrip()) :]
        body = cell.strip()
        translated.append(f"{left}{translate_text(body, target) if body else body}{right}")
    return f"{leading}{'|'.join(translated)}{trailing}"


def translate_line(line: str, target: str) -> str:
    if not line.strip():
        return line

    if line.lstrip().startswith("|"):
        return translate_table_row(line, target)

    match = re.match(r"^(\s{0,3}#{1,6}\s+)(.+)$", line)
    if match:
        return f"{match.group(1)}{translate_text(match.group(2), target)}"

    match = re.match(r"^(\s*[-*]\s+)(.+)$", line)
    if match:
        return f"{match.group(1)}{translate_text(match.group(2), target)}"

    return translate_text(line, target)


def translate_markdown(text: str, target: str) -> str:
    output = []
    in_code_block = False

    for line in text.splitlines():
        if line.strip().startswith("```"):
            in_code_block = not in_code_block
            output.append(line)
            continue

        if in_code_block:
            output.append(line)
        else:
            output.append(translate_line(line, target))

    return "\n".join(output) + "\n"


def write_translation_files(source: Path, output_dir: Path, languages: list[str]) -> None:
    source_text = source.read_text(encoding="utf-8")
    output_dir.mkdir(parents=True, exist_ok=True)

    for language in languages:
        translated = translate_markdown(source_text, language)
        output_path = output_dir / f"{source.stem}.{language}{source.suffix}"
        output_path.write_text(translated, encoding="utf-8", newline="\n")


def write_combined_document(source: Path, output: Path, languages: list[str]) -> None:
    source_text = source.read_text(encoding="utf-8")
    sections = [f"## 日本語\n\n{source_text.strip()}\n"]

    for language in languages:
        translated = translate_markdown(source_text, language).strip()
        sections.append(f"## {LANGUAGES[language]}\n\n{translated}\n")

    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text("\n---\n\n".join(sections), encoding="utf-8", newline="\n")


def main() -> None:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)

    files_parser = subparsers.add_parser("files")
    files_parser.add_argument("source", type=Path)
    files_parser.add_argument("output_dir", type=Path)
    files_parser.add_argument("--languages", nargs="+", default=list(LANGUAGES))

    combined_parser = subparsers.add_parser("combined")
    combined_parser.add_argument("source", type=Path)
    combined_parser.add_argument("output", type=Path)
    combined_parser.add_argument("--languages", nargs="+", default=list(LANGUAGES))

    args = parser.parse_args()
    languages = [language for language in args.languages if language in LANGUAGES]

    if args.command == "files":
        write_translation_files(args.source, args.output_dir, languages)
    else:
        write_combined_document(args.source, args.output, languages)


if __name__ == "__main__":
    main()
