use crate::{
    backend::Backend,
    buffer::Cell,
    layout::Rect,
    style::{Color, Modifier},
};
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    execute, queue,
    style::{
        Attribute as CAttribute, Color as CColor, SetAttribute, SetBackgroundColor,
        SetForegroundColor,
    },
    terminal::{self, Clear, ClearType},
};
use std::io::{self, Write};

#[cfg(not(target_arch = "wasm32"))]
use crossterm::style::Print;

use core::marker::PhantomData;
#[cfg(target_arch = "wasm32")]
use xterm_js_sys::{crossterm_support::XtermJsCrosstermBackend, xterm::Terminal};

#[cfg(not(target_arch = "wasm32"))]
pub struct CrosstermBackend<'a, W: Write> {
    buffer: W,
    _a: PhantomData<&'a ()>,
}

#[cfg(target_arch = "wasm32")]
pub struct CrosstermBackend<'a, W: Write = Vec<u8>> {
    pub buffer: XtermJsCrosstermBackend<'a>,
    _w: PhantomData<W>,
}

impl<'a, W> CrosstermBackend<'a, W>
where
    W: Write,
{
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(buffer: W) -> Self {
        Self {
            buffer,
            _a: PhantomData,
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn new(terminal: &'a Terminal) -> CrosstermBackend<'a, W> {
        Self {
            buffer: terminal.into(),
            _w: PhantomData,
        }
    }
}

impl<'a, W> Write for CrosstermBackend<'a, W>
where
    W: Write,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buffer.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.buffer.flush()
    }
}

impl<'t, W> Backend for CrosstermBackend<'t, W>
where
    W: Write,
{
    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        let mut fg = Color::Reset;
        let mut bg = Color::Reset;
        let mut modifier = Modifier::empty();
        let mut last_pos: Option<(u16, u16)> = None;
        for (x, y, cell) in content {
            // Move the cursor if the previous location was not (x - 1, y)
            if !matches!(last_pos, Some(p) if x == p.0 + 1 && y == p.1) {
                map_error(queue!(self.buffer, MoveTo(x, y)))?;
            }
            last_pos = Some((x, y));
            if cell.modifier != modifier {
                let diff = ModifierDiff {
                    from: modifier,
                    to: cell.modifier,
                };
                diff.queue(&mut self.buffer)?;
                modifier = cell.modifier;
            }
            if cell.fg != fg {
                let color = CColor::from(cell.fg);
                map_error(queue!(self.buffer, SetForegroundColor(color)))?;
                fg = cell.fg;
            }
            if cell.bg != bg {
                let color = CColor::from(cell.bg);
                map_error(queue!(self.buffer, SetBackgroundColor(color)))?;
                bg = cell.bg;
            }

            map_error(queue!(self.buffer, Print(&cell.symbol)))?;
        }

        #[cfg(not(target_arch = "wasm32"))]
        let res = queue!(self.buffer, Print(string));

        #[cfg(target_arch = "wasm32")]
        let res = self
            .buffer
            .write_immediately(string)
            .map_err(crossterm::ErrorKind::IoError);

        let res = res.and_then(|()| {
            queue!(
                self.buffer,
                SetForegroundColor(CColor::Reset),
                SetBackgroundColor(CColor::Reset),
                SetAttribute(CAttribute::Reset),
            )
        });

        map_error(res)
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        map_error(execute!(self.buffer, Hide))
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        map_error(execute!(self.buffer, Show))
    }

    fn get_cursor(&mut self) -> io::Result<(u16, u16)> {
        #[cfg(not(target_arch = "wasm32"))]
        let res = crossterm::cursor::position()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()));

        #[cfg(target_arch = "wasm32")]
        let res = crossterm::cursor::position(&self.buffer.terminal)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()));
        res
    }

    fn set_cursor(&mut self, x: u16, y: u16) -> io::Result<()> {
        map_error(execute!(self.buffer, MoveTo(x, y)))
    }

    fn clear(&mut self) -> io::Result<()> {
        map_error(execute!(self.buffer, Clear(ClearType::All)))
    }

    fn size(&self) -> io::Result<Rect> {
        #[cfg(not(target_arch = "wasm32"))]
        let (width, height) =
            terminal::size().map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

        #[cfg(target_arch = "wasm32")]
        let (width, height) = terminal::size(self.buffer.terminal)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

        Ok(Rect::new(0, 0, width, height))
    }

    fn flush(&mut self) -> io::Result<()> {
        self.buffer.flush()
    }
}

fn map_error(error: crossterm::Result<()>) -> io::Result<()> {
    error.map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
}

impl From<Color> for CColor {
    fn from(color: Color) -> Self {
        match color {
            Color::Reset => CColor::Reset,
            Color::Black => CColor::Black,
            Color::Red => CColor::DarkRed,
            Color::Green => CColor::DarkGreen,
            Color::Yellow => CColor::DarkYellow,
            Color::Blue => CColor::DarkBlue,
            Color::Magenta => CColor::DarkMagenta,
            Color::Cyan => CColor::DarkCyan,
            Color::Gray => CColor::Grey,
            Color::DarkGray => CColor::DarkGrey,
            Color::LightRed => CColor::Red,
            Color::LightGreen => CColor::Green,
            Color::LightBlue => CColor::Blue,
            Color::LightYellow => CColor::Yellow,
            Color::LightMagenta => CColor::Magenta,
            Color::LightCyan => CColor::Cyan,
            Color::White => CColor::White,
            Color::Indexed(i) => CColor::AnsiValue(i),
            Color::Rgb(r, g, b) => CColor::Rgb { r, g, b },
        }
    }
}

#[derive(Debug)]
struct ModifierDiff {
    pub from: Modifier,
    pub to: Modifier,
}

#[cfg(any(unix, target_arch = "wasm32"))]
impl ModifierDiff {
    fn queue<W>(&self, mut w: W) -> io::Result<()>
    where
        W: io::Write,
    {
        //use crossterm::Attribute;
        let removed = self.from - self.to;
        if removed.contains(Modifier::REVERSED) {
            map_error(queue!(w, SetAttribute(CAttribute::NoReverse)))?;
        }
        if removed.contains(Modifier::BOLD) {
            map_error(queue!(w, SetAttribute(CAttribute::NormalIntensity)))?;
            if self.to.contains(Modifier::DIM) {
                map_error(queue!(w, SetAttribute(CAttribute::Dim)))?;
            }
        }
        if removed.contains(Modifier::ITALIC) {
            map_error(queue!(w, SetAttribute(CAttribute::NoItalic)))?;
        }
        if removed.contains(Modifier::UNDERLINED) {
            map_error(queue!(w, SetAttribute(CAttribute::NoUnderline)))?;
        }
        if removed.contains(Modifier::DIM) {
            map_error(queue!(w, SetAttribute(CAttribute::NormalIntensity)))?;
        }
        if removed.contains(Modifier::CROSSED_OUT) {
            map_error(queue!(w, SetAttribute(CAttribute::NotCrossedOut)))?;
        }
        if removed.contains(Modifier::SLOW_BLINK) || removed.contains(Modifier::RAPID_BLINK) {
            map_error(queue!(w, SetAttribute(CAttribute::NoBlink)))?;
        }

        let added = self.to - self.from;
        if added.contains(Modifier::REVERSED) {
            map_error(queue!(w, SetAttribute(CAttribute::Reverse)))?;
        }
        if added.contains(Modifier::BOLD) {
            map_error(queue!(w, SetAttribute(CAttribute::Bold)))?;
        }
        if added.contains(Modifier::ITALIC) {
            map_error(queue!(w, SetAttribute(CAttribute::Italic)))?;
        }
        if added.contains(Modifier::UNDERLINED) {
            map_error(queue!(w, SetAttribute(CAttribute::Underlined)))?;
        }
        if added.contains(Modifier::DIM) {
            map_error(queue!(w, SetAttribute(CAttribute::Dim)))?;
        }
        if added.contains(Modifier::CROSSED_OUT) {
            map_error(queue!(w, SetAttribute(CAttribute::CrossedOut)))?;
        }
        if added.contains(Modifier::SLOW_BLINK) {
            map_error(queue!(w, SetAttribute(CAttribute::SlowBlink)))?;
        }
        if added.contains(Modifier::RAPID_BLINK) {
            map_error(queue!(w, SetAttribute(CAttribute::RapidBlink)))?;
        }

        Ok(())
    }
}
