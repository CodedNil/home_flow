<h1 align="center">
    Home Flow
    <br>
    <a href="https://github.com/CodedNil/home_flow/blob/master/LICENSE"><img src="https://img.shields.io/github/license/CodedNil/home_flow"/></a>
    <a href="https://deps.rs/repo/github/CodedNil/home_flow"><img src="https://deps.rs/repo/github/CodedNil/home_flow/status.svg"/></a>
    <img src="https://img.shields.io/github/commit-activity/w/CodedNil/home_flow"/>
    <img src="https://img.shields.io/github/last-commit/CodedNil/home_flow"/>
    <img src="https://img.shields.io/github/actions/workflow/status/CodedNil/home_flow/rust.yml"/>
    <br>
    <img src="https://img.shields.io/github/repo-size/CodedNil/home_flow"/>
    <img src="https://img.shields.io/github/languages/code-size/CodedNil/home_flow"/>
</h1>

**HomeFlow** is a modern web application written in pure Rust, designed to integrate seamlessly with [Home Assistant](https://www.home-assistant.io/) to provide an interactive and animated map-based control of your smart home environment.

![image](https://github.com/user-attachments/assets/1786f101-76a6-4efa-8b29-19273864bd9c)

## Overview

HomeFlow visualises your home layout and allows you to interact with smart devices in a user-friendly manner. This application bridges the gap between complex smart home setups and intuitive control, ensuring that managing your smart devices is as easy as tapping on a map.

## Features

- **Interactive Map Display:** Visualise your home layout with a detailed map that shows the locations of smart devices.
- **Device Control:** Easily turn lights and other smart devices on or off directly from the map by tapping on their icons.
- **Dynamic Animations:** Enjoy a clean, animated interface that provides real-time updates and feedback on device status and actions.
- **Home Assistant Integration:** Seamlessly connect to Home Assistant to access and control all your compatible devices.
- **Web Application:** Access HomeFlow from any device with a web browser, ensuring a consistent and responsive experience across desktops, tablets, and smartphones.
- **GUI Edit Mode:** Use intuitive tools to rapidly draw out your house with ease, making it simple to update and adjust your home layout.
- **Powered by `egui`:** Utilises the [`egui`](https://github.com/emilk/egui) library for a smooth and efficient graphical user interface experience.


## Getting Started

### Installation and Configuration
1. **Clone the Repository**
2. **Install the WebAssembly Target:** `rustup target add wasm32-unknown-unknown`
3. **Install Trunk [Trunk](https://github.com/trunk-rs/trunk) for building WASM applications** `cargo install --locked trunk`
4. **Install [Just](https://github.com/casey/just) for managing build commands:** `cargo install --locked just`
5. **Create Configuration File:** Copy the `.env-template` to `.env` and fill in your Home Assistant details

### Build and Run Commands
- **Run the App in Desktop Mode:** `just`
- **Compile for WebAssembly:** `just build-web` or in release mode `just build-web-release`
- **Start the Server:** `just serve` or in release mode `just serve-release`

## Contributing
Contributions are welcome! If you'd like to contribute to HomeFlow, please fork the repository and submit a pull request with your improvements or bug fixes.
