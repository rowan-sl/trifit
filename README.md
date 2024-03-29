# Trifit, a image compression / art program

Trifit takes input images like these

<img src="./doc/assets/in/aroura_sky.jpg" width="400">
<img src="./doc/assets/in/fire.jpg" width="400">

and give you beautefull triangle versions

<img src="./doc/assets/out/aroura_sky.png" width="400">
<img src="./doc/assets/out/fire.png" width="400">

and finally, it can create mindustry logic to display the images it creates `--format mindustry` like so:

<img src="./doc/assets/out/mindustry-tree.png" width="400">

this can be used to get good looking logic images with VERY few processors (just make shure to use `--image-size 176` or `--image-size 80`)

note: *schematics are not auto-generated, only code is. this will improve in the future*

## Usage

Trifit is a CLI program. to use, pass the path to the input image, and specify the size of the output image, the size of the starting triangle grid, the number of iterations, and the ammount to shift each vertex each step. for example:

`trifit img/aroura_sky.jpg --image-size 900 --tri-size 15 --iterations 50 --shift 0.5`

for more details about useage, run `trifit --help`

### Output formats

you can also specify a file to write the result to, and its format with `--format`. available formats are `svg`, `image` (type determined by file extension), and `mindustry`

when outputing to the `mindustry` format, it will produce multiple files if it gets too long. it will draw to the `display1` output.

### Visualizations

as the program runs, it will open a window to display its progress (it also helps aleviate boredom while the program runs). if this is not desierable, use `--no-visuals` to disable this.

### Scoring methods

these affect what method is used to quantify how well a given triangle represents its part of an image, and can be specified with the `--scoring` flag.

## Installation

### From Source

To build trifit from source, clone this repo and do `cargo build` (or `cargo build --release`). the binary will end up in `./target/debug|release/trifit`

### From crates.io

`cargo install trifit`

## Contribution

Contrbuting is appreciated, feel free to open an issue or pull request. Note that any contribution submitted for inclusion in the project will be licensed according to the terms given in [LICENSE](LICENSE)

## MSRV (Minimum supported rust version)

This crate requires at least rust stable `1.62.1`
