# rascii
tool to create ascii art style media, written in rust.

## Example output:
![image example](https://github.com/anytarseir67/rascii/blob/master/examples/example.png?raw=true)

## Command line usage:
```
rascii.exe [OPTIONS] --in-media <IN_MEDIA> --out-media <OUT_MEDIA>

Options:
  -w, --width <WIDTH>              [default: 80]
  -c, --char-size <CHAR_SIZE>      [default: 23]
  -b, --back-darken <BACK_DARKEN>  [default: 0.7]
  -v, --vcodec <VCODEC>            [default: libx264]
  -i, --in-media <IN_MEDIA>
  -o, --out-media <OUT_MEDIA>
  -h, --help                       Print help
```

## Attribution
this repository features sections of code pulled from [cv-convert](https://github.com/jerry73204/rust-cv-convert), to work around issues with the crate itself.