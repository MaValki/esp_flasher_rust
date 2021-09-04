#![allow(dead_code)]
#![allow(unused_variables)]

use crate::port::*;
use crate::EspLoaderError::*;
use crate::loader_port_serial_read;

pub enum EspLoaderError {
    Fail,             // Unspecified error
    Timeout,          // Timeout elapsed
    ImageSize,        // Image size to flash is larger than flash size
    InvalidMD5,       // Computed and received MD5 does not match
    // InvalidParam,     // Invalid parameter passed to function
    InvalidTarget,    // Connected target is invalid
    UnsupportedChip,  // Attached chip is not supported
    InvalidResponse,  // Internal error
}

const DELIMITER: u8 = 0xc0;
const C0_REPLACEMENT: [u8; 2] = [0xdb, 0xdc];
const BD_REPLACEMENT: [u8; 2] = [0xdb, 0xdd];

fn serial_read_char() -> Result<u8, EspLoaderError>
{
	let mut ch: [u8; 1] = [ 0 ]; 
    loader_port_serial_read(&mut ch, loader_port_remaining_time())?;
    Ok(ch[0] as u8)
}

fn serial_read(buff: &mut [u8]) -> Result<(), EspLoaderError>
{
    loader_port_serial_read(buff, loader_port_remaining_time())
}

fn serial_write(buff: &[u8]) -> Result<(), EspLoaderError>
{
    loader_port_serial_write(buff, loader_port_remaining_time())
}

fn slip_receive_data(buff: &mut [u8]) -> Result<(), EspLoaderError>
{
    for item in buff.iter_mut() {
        match serial_read_char()? {
        	0xdb =>	match serial_read_char()? {
        		0xdc => *item = 0xc0,
        		0xdd => *item = 0xbd,
        		_ => return Err(InvalidResponse), 
        	}
        	ch@ _ => *item = ch,
        }
    }
    Ok(())
}

slip_receive_packet(uint8_t *buff, uint32_t size)
{
    while serial_read_char() != DELIMITER { }

    // Workaround: bootloader sends two dummy(0xc0) bytes after response when baud rate is changed.
    while let ch = serial_read_char()? {
    	if ch != DELIMITER {
    		buff[0] = ch;
    		break;
    	}
    }

    let ch = loop {
    	let ch = serial_read_char()?
    	if  ch != DELIMITER { break ch; }
    }

    slip_receive_data(&mut buff[1..]) );

    // Delimiter
    RETURN_ON_ERROR( serial_read(&ch, 1) );
    if serial_read_char()? != DELIMITER {
        return Err(InvalidResponse)
    }

    Ok(())
}


pub fn loader_sync_cmd() -> Result<(), EspLoaderError>
{
	Ok(())
}

pub fn loader_spi_attach_cmd(config: u32) -> Result<(), EspLoaderError>
{
	Ok(())
}

pub fn loader_write_reg_cmd(address: u32, value: u32, mask: u32, delay_us: u32) -> Result<(), EspLoaderError>
{
	Ok(())
}

pub fn loader_read_reg_cmd(address: u32) -> Result<u32, EspLoaderError>
{
	Ok(0)
}

pub fn loader_spi_parameters(total_size: u32) -> Result<(), EspLoaderError>
{
	Ok(())
}

pub fn loader_flash_begin_cmd(
	offset: u32, 
	erase_size: u32, 
	block_size: u32,
	blocks_to_write: u32) -> Result<(), EspLoaderError>
{
	Ok(())
}

pub fn loader_flash_data_cmd(data: &[u8]) -> Result<(), EspLoaderError>
{
	Ok(())
}

pub fn loader_flash_end_cmd(stay_in_bootloader: bool) -> Result<(), EspLoaderError>
{
	Ok(())
}
pub fn loader_change_baudrate_cmd(baud: u32) -> Result<(), EspLoaderError>
{
	Ok(())
}
pub fn loader_md5_cmd(addr: u32, size: u32) -> Result<[u8; 32], EspLoaderError>
{
	let res: [u8; 32] = [0; 32];
	Ok(res)
}