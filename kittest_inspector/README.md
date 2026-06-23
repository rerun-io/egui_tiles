# Kittest inspection utilities

This repo contains two tools using the [egui_inspection](https://github.com/emilk/egui/blob/main/crates/egui_inspection/README.md) 
protocol. Today, these are only supported by egui, but the idea is that these could be used by other rust ui 
frameworks using kittest and accesskit in the future. 

## egui_mcp

[egui_mcp](./crates/egui_mcp) is a mcp that can be used by an agent to connect and interact with egui apps. Useful
to have the agent verify it's work, reproduce a bug or test an app.

## kittest_inspector

[kittest_inspector](./crates/kittest_inspector) is a gui that can be used to inspect kittest tests and step through 
them frame-by-frame. It's not ready for use yet (the required inspection protocol features haven't been merged yet).
