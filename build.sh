# See https://stackoverflow.com/questions/9853325/how-to-convert-a-svg-to-a-png-with-imagemagick
inkscape -w 32 packaging/icon.svg -o packaging/icon32x32.png
inkscape -w 64 packaging/icon.svg -o packaging/icon64x64.png
inkscape -w 256 packaging/icon.svg -o packaging/icon256x256.png
inkscape -w 512 packaging/icon.svg -o packaging/icon512x512.png
