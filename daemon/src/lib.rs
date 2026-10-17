//! TCA-NEO bridge daemon: TCA Sidestick + Quadrant → virtual T.Flight Hotas NEO (driver/neohid).
pub mod app;
pub mod config;
pub mod device;
pub mod driver;
pub mod emulation;
pub mod ffi;
pub mod hidhide;
pub mod http;
pub mod labels;
pub mod neo;
pub mod process;
pub mod protocol;
pub mod shared;
pub mod tca;
pub mod tray;
pub mod validation;
