use std::io::{Result, Write};

use quick_xml::{
    events::{attributes::Attribute, BytesDecl, BytesEnd, BytesStart, BytesText, Event},
    Writer,
};

pub struct XMLWriter<W: Write> {
    writer: Writer<W>,
}

impl<W: Write> XMLWriter<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer: Writer::new_with_indent(writer, b' ', 2),
        }
    }

    pub fn write_declaration(&mut self) -> Result<()> {
        self.writer
            .write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))?;

        Ok(())
    }

    pub fn open_no_attributes(&mut self, tag: &str) -> Result<()> {
        self.writer
            .write_event(Event::Start(BytesStart::new(tag)))?;

        Ok(())
    }

    pub fn open<'a, A, I>(&mut self, tag: &str, attributes: I) -> Result<()>
    where
        A: Into<Attribute<'a>>,
        I: IntoIterator<Item = A>,
    {
        let mut node = BytesStart::new(tag);

        for attribute in attributes {
            node.push_attribute(attribute);
        }

        self.writer.write_event(Event::Start(node))?;

        Ok(())
    }

    pub fn open_empty<'a, A, I>(&mut self, tag: &str, attributes: I) -> Result<()>
    where
        A: Into<Attribute<'a>>,
        I: IntoIterator<Item = A>,
    {
        let mut node = BytesStart::new(tag);

        for attribute in attributes {
            node.push_attribute(attribute);
        }

        self.writer.write_event(Event::Empty(node))?;

        Ok(())
    }

    pub fn write_text(&mut self, text: &str) -> Result<()> {
        self.writer.write_event(Event::Text(BytesText::new(text)))?;

        Ok(())
    }

    pub fn close(&mut self, tag: &str) -> Result<()> {
        self.writer.write_event(Event::End(BytesEnd::new(tag)))?;

        Ok(())
    }

    pub fn writeln(&mut self) -> Result<()> {
        self.writer.get_mut().write_all(b"\n")?;

        Ok(())
    }
}
