# BakinRbrConverter

## Introduction
This is a CLI tool for converting between RPG Developer Bakin's `.rbr` files and JSON format, primarily designed for extracting text for translation purpose. Some section of the files are not studied and are unknown, so they're left untouched and kept as hexadecimal bytes.


## Usage

### Parsing (RBR → JSON)
To parse all `.rbr` files in directory into `.json` files, use the following command:
```
bakin-rbr-converter.exe parse --input <INPUT> --output <OUTPUT> --clean
```
- `<INPUT>`: The directory containing all the `.rbr` files you want to parse
- `<OUTPUT>`: The directory where the parsed files will be saved. The directory structure is preserved
- `--clean`: Optional - Empty output directory before processing

### Encoding (JSON → RBR)
To recompile all `.json` files in directory into `.rbr` files, use the following command:
```
bakin-rbr-converter.exe encode [OPTIONS] --input <INPUT> --output <OUTPUT> --clean
```
- `<INPUT>`: The directory containing all the `.rbr` files you want to parse
- `<OUTPUT>`: The directory where the parsed files will be saved. The directory structure is preserved
- `--clean`: Optional - Empty output directory before processing

## Contributing
I used ImHex to study the file structures, so i've included the pattern file inside `/imhex` directory.

Some section of the file structure are known but not impemented(Entity Header) in the parser because it's not necessary for text translation.

A LOT of sections outside of map files are unknown. i'll gladly accept any contribution.


## License
This project is licensed under the MIT License. See the [LICENSE](./LICENSE) file for details.