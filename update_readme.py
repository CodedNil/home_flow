import re
import sys

def format_size(size_in_bytes):
    size_in_bytes = int(size_in_bytes)
    # Check if size is smaller than 1 MB
    if size_in_bytes < 1000 * 1000:
        # Convert size to kilobytes and format
        size_in_kb = size_in_bytes / 1000
        return f"{size_in_kb:.2f}".rstrip('0').rstrip('.') + ' KB'
    else:
        # Convert size to megabytes and format
        size_in_mb = size_in_bytes / (1000 * 1000)
        return f"{size_in_mb:.2f}".rstrip('0').rstrip('.') + ' MB'


def update_badge(content, badge_name, size_in_bytes):
    formatted_size = format_size(size_in_bytes)
    pattern = fr"(https://img.shields.io/badge/{badge_name}-)(.*?)(-blue)"
    return re.sub(pattern, lambda match: match.group(1) + formatted_size + match.group(3), content)

if __name__ == "__main__":
    if len(sys.argv) != 3:
        print("Usage: python update_readme.py <server binary size> <wasm file size>")
        sys.exit(1)

    with open("README.md", "r") as file:
        content = file.read()

    content = update_badge(content, "binary_size", sys.argv[1])
    content = update_badge(content, "wasm_size", sys.argv[2])

    with open("README.md", "w") as file:
        file.write(content)
