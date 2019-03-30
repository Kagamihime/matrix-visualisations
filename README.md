# Matrix Visualisations

A tool for real-time visualisation of the events graph of a Matrix room from
the perspective of one or more HSes.

## Build and run

Clone or download this repository.

To run this project you need to have [cargo-web] installed:

    $ cargo install cargo-web

> Add `--force` option to ensure you install the latest version.

To run the project use:

    $ cargo web start --release

## Usage

1. Enter your HS address, username, password and the **ID** of a room to
observe (the **ID**, not an **alias**) in the input fields.

2. Click on the button `Connect` and wait for the graph to appear (note that
you can have a look at the web console to get more feedbacks from the
application).

3. Click on the button `Disconnect` to close the session opened by the
application.
