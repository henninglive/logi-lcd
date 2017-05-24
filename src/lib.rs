extern crate logi_lcd_sys as sys;

pub use sys::LcdButton;
pub use sys::BitFlags;
pub use sys::MONO_WIDTH;
pub use sys::MONO_HEIGHT;
pub use sys::MONO_BYTES_PER_PIXEL;
pub use sys::COLOR_WIDTH;
pub use sys::COLOR_HEIGHT;
pub use sys::COLOR_BYTES_PER_PIXEL;

use sys::LcdType as LcdTypeFlags;

use std::marker::PhantomData;
use std::os::raw::c_int;
use std::sync::atomic::{AtomicBool, Ordering, ATOMIC_BOOL_INIT};
use std::error::Error;
use std::fmt::{Display, Formatter};

static INITIALIZED: AtomicBool = ATOMIC_BOOL_INIT;

pub trait LcdType {}
pub trait LcdMono: LcdType {}
pub trait LcdColor: LcdType {}

pub struct LcdTypeMono;
pub struct LcdTypeColor;
pub struct LcdTypeBoth;

pub struct Lcd<T: LcdType> {
    type_flags: u32,
    type_data: PhantomData<T>,
}

#[derive(Debug)]
pub enum LcdError {
    NotConnected,
    Initialization,
    MonoBackground,
    MonoText,
    ColorBackground,
    ColorTitle,
    ColorText,
    NullCharacter,
}

impl LcdType for LcdTypeMono {}
impl LcdType for LcdTypeColor {}
impl LcdType for LcdTypeBoth {}

impl LcdMono for LcdTypeMono {}
impl LcdColor for LcdTypeColor {}
impl LcdMono for LcdTypeBoth {}
impl LcdColor for LcdTypeBoth {}

impl Error for LcdError {
    fn description(&self) -> &str {
        match *self {
            LcdError::NotConnected    => "LCD is not connected",
            LcdError::Initialization  => "A FFI call to LogiLcdInit() has failed",
            LcdError::MonoBackground  => "A FFI call to LogiLcdMonoSetBackground() has failed",
            LcdError::MonoText        => "A FFI call to LogiLcdMonoSetText() has failed",
            LcdError::ColorBackground => "A FFI call to LogiLcdColorSetBackground() has failed",
            LcdError::ColorTitle      => "A FFI call to LogiLcdColorSetTitle() has failed",
            LcdError::ColorText       => "A FFI call to LogiLcdColorSetText() has failed",
            LcdError::NullCharacter   => "Unexpected NULL character",
        }
    }
}

impl Display for LcdError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "LcdError: {}", self.description())
    }
}

fn str_to_wchar(s: &str) -> Result<Vec<u16>, LcdError> {
    let mut v = s.encode_utf16().collect::<Vec<u16>>();

    if v.iter().any(|&val| val == 0) {
        return Err(LcdError::NullCharacter);
    }

    v.push(0);

    Ok(v)
}

unsafe fn init(app_name: &str, type_flags: u32) -> Result<(), LcdError> {
    assert_eq!(INITIALIZED.swap(true, Ordering::SeqCst), false);
    let ws = str_to_wchar(app_name)?;

    let ret = match sys::LogiLcdInit(ws.as_ptr(), type_flags) {
        true => {
            match sys::LogiLcdIsConnected(type_flags) {
                true => Ok(()),
                false => Err(LcdError::NotConnected),
            }
        },
        false => Err(LcdError::Initialization),
    };
    if ret.is_err() {
        INITIALIZED.store(false, Ordering::SeqCst);
    }
    ret
}


/// Initialize and connect to a monochrome lcd device.
///
/// ### Parameters:
/// - app_name: The name of your applet.
///
/// ### Panics
/// Will panic if another lcd instance exits.
///
pub fn init_mono(app_name: &str) -> Result<Lcd<LcdTypeMono>, LcdError>  {
    let type_flags: u32 = LcdTypeFlags::MONO.into();
    unsafe {
        init(app_name, type_flags).map(|_| Lcd {
            type_flags: type_flags, 
            type_data: PhantomData,
        })
    }
}

/// Initialize and connect to a color lcd device.
///
/// ### Parameters:
/// - app_name: The name of your applet.
///
/// ### Panics
/// Will panic if another lcd instance exits.
///
pub fn init_color(app_name: &str) -> Result<Lcd<LcdTypeColor>, LcdError> {
    let type_flags: u32 = LcdTypeFlags::COLOR.into();
    unsafe {
        init(app_name, type_flags).map(|_| Lcd {
            type_flags: type_flags, 
            type_data: PhantomData,
        })
    }
}

/// Initialize and connect to either a monochrome or color lcd device.
///
/// ### Parameters:
/// - app_name: The name of your applet.
///
/// ### Panics
/// Will panic if another lcd instance exits.
///
pub fn init_either(app_name: &str) -> Result<Lcd<LcdTypeBoth>, LcdError> {
    let type_flags = LcdTypeFlags::either().bits();
    unsafe {
        init(app_name, type_flags).map(|_| Lcd {
            type_flags: type_flags, 
            type_data: PhantomData,
        })
    }
}

impl<T: LcdType> Lcd<T> {
    /// Checks if the device is connected.
    ///
    /// ### Return value:
    /// If a device supporting the lcd type specified is found, it returns `true`. Otherwise `false`
    ///
    pub fn is_connected(&self) -> bool {
        unsafe {
            sys::LogiLcdIsConnected(self.type_flags)
        }
    }

    /// Updates the lcd display.
    ///
    /// ### Notes:
    /// You have to call this function every frame of your main loop, to keep the lcd updated.
    ///
    pub fn update(&mut self) {
        unsafe {
            sys::LogiLcdUpdate();
        }
    }
}

impl<T: LcdType + LcdMono> Lcd<T> {
    /// Checks if the buttons specified by the parameter are being pressed.
    ///
    /// ### Return value:
    /// If the button specified is being pressed it returns `true`. Otherwise `false`
    ///
    /// ### Notes:
    /// The button will be considered pressed only if your applet is the one currently in the foreground.
    ///
    pub fn is_mono_buttons_pressed(&self, buttons: BitFlags<LcdButton>) -> bool {
        unsafe {
            sys::LogiLcdIsButtonPressed((buttons & LcdButton::mono()).bits())
        }
    }

    /// Sets the specified image as background for the monochrome lcd device.
    ///
    /// ### Parameters:
    /// - bytemap: The image data is organized as a rectangular area, 160 bytes wide and 43
    /// bytes high. Despite the display being monochrome, 8 bits per pixel are used
    /// here for simple manipulation of individual pixels. The SDK will turn on the
    /// pixel on the screen if the value assigned to that byte is >= 128, it will
    /// remain off if the value is < 128.
    ///
    /// ### Panics
    /// Will panic if bytemaps size is not 160x43bytes long.
    ///
    pub fn set_mono_background(&mut self, bytemap: &[u8]) -> Result<(), LcdError> {
        assert_eq!(bytemap.len(), MONO_WIDTH * MONO_HEIGHT);
        unsafe {
            match sys::LogiLcdMonoSetBackground(bytemap.as_ptr()) {
                true => Ok(()),
                false => Err(LcdError::MonoBackground),
            }
        }
    }

    /// Sets the specified text in the requested line on the monochrome lcd device.
    ///
    /// ### Parameters:
    /// - line_number: The line on the screen you want the text to appear. The monochrome lcd display
    ///   has 4 lines, so this parameter can be any number from 0 to 3.
    /// - text: Defines the text you want to display
    ///
    /// ### Panics
    /// Will panic if line_number larger than or equal to 4.
    ///
    pub fn set_mono_text(&mut self, line_number: usize, text: &str) -> Result<(), LcdError> {
        let ws = str_to_wchar(text)?;
        assert!(line_number < 4);
        unsafe {
            match sys::LogiLcdMonoSetText(line_number as c_int, ws.as_ptr()) {
                true => Ok(()),
                false => Err(LcdError::MonoText),
            }
        }
    }
}

impl<T: LcdType + LcdColor> Lcd<T> {
    /// Checks if the buttons specified by the parameter are being pressed.
    ///
    /// ### Return value:
    /// If the button specified is being pressed it returns `true`. Otherwise `false`
    ///
    /// ### Notes:
    /// The button will be considered pressed only if your applet is the one currently in the foreground.
    ///
    pub fn is_color_buttons_pressed(&self, buttons: BitFlags<LcdButton>) -> bool {
        unsafe {
            sys::LogiLcdIsButtonPressed((buttons & LcdButton::color()).bits())
        }
    }

    pub fn set_color_background(&mut self, bitmap: &[u8]) -> Result<(), LcdError> {
        assert_eq!(bitmap.len(), COLOR_WIDTH * COLOR_HEIGHT * COLOR_BYTES_PER_PIXEL);
        unsafe {
            match sys::LogiLcdColorSetBackground(bitmap.as_ptr()) {
                true => Ok(()),
                false => Err(LcdError::ColorBackground),
            }
        }
    }

    pub fn set_color_title(&mut self, text: &str, red: u8, green: u8, blue: u8)
        -> Result<(), LcdError>
    {
        let ws = str_to_wchar(text)?;

        unsafe {
            match sys::LogiLcdColorSetTitle(ws.as_ptr(), red as c_int,
                green as c_int, blue as c_int)
            {
                true  => Ok(()),
                false => Err(LcdError::ColorTitle),
            }
        }
    }

    pub fn set_color_text(&mut self, line_number: usize, text: &str,
        red: u8, green: u8, blue: u8) -> Result<(), LcdError>
    {
        let ws = str_to_wchar(text)?;
        assert!(line_number < 4);
        unsafe {
            match sys::LogiLcdColorSetText(line_number as c_int,
                ws.as_ptr(), red as c_int, green as c_int, blue as c_int)
            {
                true => Ok(()),
                false => Err(LcdError::ColorText),
            }
        }
    }
}

impl<T: LcdType> Drop for Lcd<T> {
    /// Kills the applet and frees memory used by the SDK
    fn drop(&mut self) {
        unsafe {
            sys::LogiLcdShutdown();
        }
        INITIALIZED.store(false, Ordering::SeqCst);
    }
}