# Stdin

Reads lines from the standard input and fuzzy matches on those. The selected one is printed to stdout.
Allows for easy integration into scripts that have been made with something like dmenu in mind.

## Usage

This plugin should generally be used alone, if a dmenu replacement is needed. This can be done with `anyrun --plugins libstdin.so`.
The content to fuzzy match on needs to be piped into Anyrun.

## Icons and images

This plugin uses tabs to separate the text from the custom icon or image file. This means that you need to make sure that you don't pipe any tabs, unless you want to set a custom icon or image.

This feature works by adding a tab after the title text, and then either:
- specifying an icon name or path
- or specifying an image path after with the `image:` prefix

For example:
```
Option 1    help-about
Option 2    /path/to/icon.png
Option 3    image:/path/to/image.png
```
