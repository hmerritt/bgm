# bgm

Background manager.

A rust program, continually running, to manage the current OS background image.

## ⚡ Features

- Small in size, low memory footprint
- ALways scales images to fit the screen (will crop if necessary)
- `bgm.hcl` — config file
    - `sources` — Image sources. Any combination of these can be added and used at once to source images:
        - Single image path
        - Directory path
        - RSS feed
    - `timer` — Image display time, before changing to the next one
    - `remoteUpdateTimer` — How often to pull RSS feed
