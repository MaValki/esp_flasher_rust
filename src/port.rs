
#![allow(dead_code)]
#![allow(unused_variables)]

use crate::EspLoaderError;

pub fn loader_port_serial_read(buff: &mut [u8], timeout: u32) -> Result<(), EspLoaderError>
{
	Ok(())
}

pub fn loader_port_serial_write(buff: &[u8], timeout: u32) -> Result<(), EspLoaderError>
{
	Ok(())
}

pub fn loader_port_remaining_time() -> u32
{
	1u32
}

pub fn loader_port_start_timer(ms: u32)
{

}

pub fn loader_port_enter_bootloader()
{

}


pub fn loader_port_delay_ms(ms: u32)
{

}

pub fn loader_port_debug_print(msg: &str)
{
	println!("Debug: msg");
}

pub fn loader_port_reset_target()
{

}