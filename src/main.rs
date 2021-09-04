#![allow(dead_code)]

use std::cmp::max;
use std::cmp::min;

mod port;
mod comm;

use port::*;
use comm::*;
use crate::EspLoaderError::*;

const SPI_REG_BASE: u32 = 0x60000200;
const SPI_CMD_REG: u32  = SPI_REG_BASE + 0x00;
const SPI_USR_REG: u32  = SPI_REG_BASE + 0x1c;
const SPI_USR1_REG: u32 = SPI_REG_BASE + 0x20;
const SPI_USR2_REG: u32 = SPI_REG_BASE + 0x24;
const SPI_W0_REG: u32   = SPI_REG_BASE + 0x40;
const SPI_MOSI_DLEN_REG: u32 = 0;
const SPI_MISO_DLEN_REG: u32 = 0;

const UART_DATE_REG_ADDR: u32 = 0x1111; 
const UART_DATE_REG2_ADDR: u32 = 0x2222; 

const DATE_REG_1: u32 = 0x5555; 
const DATE_REG_2: u32 = 0x6666; 

const DEFAULT_TIMEOUT: u32 = 3000;
const DEFAULT_FLASH_TIMEOUT: u32 = 3000;       // timeout for most flash operations
const ERASE_REGION_TIMEOUT_PER_MB: u32 = 3000; // timeout (per megabyte) for erasing a region
const MD5_TIMEOUT_PER_MB: u32 = 800;
const PADDING_PATTERN: u8 = 0xFF;

static mut S_FLASH_WRITE_SIZE: usize = 0;

const MEGABYTE: u32 = 1024 * 1024;

const SIZE_ID_TO_FLASH_SIZE: [u32; 7] = [
    MEGABYTE / 4,  // 256KB
    MEGABYTE / 2,  // 512KB
    1 * MEGABYTE,  // 1MB
    2 * MEGABYTE,  // 2MB
    4 * MEGABYTE,  // 4MB
    8 * MEGABYTE,  // 8MB
    16 * MEGABYTE, // 16MB
];

fn timeout_per_mb(size_bytes: u32, timeout: u32) -> u32
{
    let timeout = timeout * (size_bytes / MEGABYTE);
    max(timeout, DEFAULT_FLASH_TIMEOUT)
}

fn detect_chip() -> Result<(), EspLoaderError>
{
    let reg_1 = esp_loader_read_register(UART_DATE_REG_ADDR)?;
    let reg_2 = esp_loader_read_register(UART_DATE_REG2_ADDR)?;

    if DATE_REG_1 == reg_1 && (DATE_REG_2 == 0 || DATE_REG_2 == reg_2) {
        Ok(())
    } else {
        Err(InvalidTarget)
    }
}

pub struct ConnectArgs 
{
  sync_timeout: u32,  // Maximum time to wait for response from serial interface.
  trials: i32,        // Number of trials to connect to target. If greater than 1,
                      // 100 millisecond delay is inserted after each try.
}

fn start_timer_default() { 
    loader_port_start_timer(DEFAULT_TIMEOUT);
}

// TODO: #ifndef TARGET_ESP8266
fn attach() -> Result<(), EspLoaderError>
{
    const SPI_PIN_CONFIG_DEFAULT: u32 = 0;
    
    start_timer_default();
    loader_spi_attach_cmd(SPI_PIN_CONFIG_DEFAULT)
}

pub fn esp_loader_connect(connect_args: &ConnectArgs) -> Result<(), EspLoaderError>
{
    let mut trials = connect_args.trials;

    loader_port_enter_bootloader();

    loop {
        trials -= 1;
        loader_port_start_timer(connect_args.sync_timeout);
        match loader_sync_cmd() {
            Err(Timeout) => if trials == 0 { return Err(Timeout) },
            Err(err) => return Err(err),
            Ok(()) => break,
        }
        loader_port_delay_ms(100);
    }

    detect_chip()?;
    attach()
}

// TODO: #ifndef TARGET_ESP826
fn spi_set_data_lengths(mosi_bits: u32, miso_bits: u32) -> Result<(), EspLoaderError>
{
    if mosi_bits > 0 { esp_loader_write_register(SPI_MOSI_DLEN_REG, mosi_bits - 1)? }
    if miso_bits > 0 { esp_loader_write_register(SPI_MISO_DLEN_REG, miso_bits - 1)? }
    Ok(())
}

// TODO: #ifdef TARGET_ESP826
fn spi_set_data_lengths_2(mosi_bits: u32, miso_bits: u32) -> Result<(), EspLoaderError>
{
    let mosi_bitlen_shift: u32 = 17;
    let miso_bitlen_shift: u32 = 8;
    let mosi_mask: u32 = if mosi_bits == 0 { 0 } else { mosi_bits - 1 };
    let miso_mask: u32 = if miso_bits == 0 { 0 } else { miso_bits - 1 };
    let usr_reg: u32   = (miso_mask << miso_bitlen_shift) | 
                         (mosi_mask << mosi_bitlen_shift);

    esp_loader_write_register(SPI_USR1_REG, usr_reg)
}

enum SpiFlashCommand {
    SpiFlashReadId = 0x9F,
}

use crate::SpiFlashCommand::*;

fn spi_flash_command(cmd: SpiFlashCommand, data_tx: Option<&[u32]>, rx_size: u32) -> Result<u32, EspLoaderError>
{
    // assert(rx_size <= 32); // Reading more than 32 bits back from a SPI flash operation is unsupported
    // assert(tx_size <= 64); // Writing more than 64 bytes of data with one SPI command is unsupported
    
    const SPI_USR_CMD: u32  = 1 << 31;
    const SPI_USR_MISO: u32 = 1 << 28;
    const SPI_USR_MOSI: u32 = 1 << 27;
    const SPI_CMD_USR: u32  = 1 << 18;
    const CMD_LEN_SHIFT: u32 = 28;

    // Save SPI configuration
    let old_spi_usr = esp_loader_read_register(SPI_USR_REG)?;
    let old_spi_usr2 = esp_loader_read_register(SPI_USR2_REG)?;
    let mut tx_size = 0;
    if let Some(data) = data_tx {
        tx_size = 8 * data.len() as u32;
    }

    spi_set_data_lengths(tx_size, rx_size)?;

    let usr_reg_2 = (7 << CMD_LEN_SHIFT) | cmd as u32;
    let mut usr_reg = SPI_USR_CMD;
    if rx_size > 0 { usr_reg |= SPI_USR_MISO; }
    if tx_size > 0 { usr_reg |= SPI_USR_MOSI; }

    esp_loader_write_register(SPI_USR_REG, usr_reg)?;
    esp_loader_write_register(SPI_USR2_REG, usr_reg_2 )?;

    if tx_size == 0 {
        // clear data register before we read it
        esp_loader_write_register(SPI_W0_REG, 0)?;
    } else {
        for (i, &data) in data_tx.iter().enumerate() {
            esp_loader_write_register(SPI_W0_REG + (i * 4) as u32, data[i])?;
        }
    }

    esp_loader_write_register(SPI_CMD_REG, SPI_CMD_USR)?;

    let mut trials = 10;
    loop {
        trials -= 1;
        let reg = esp_loader_read_register(SPI_CMD_REG)?;
        if reg & SPI_CMD_USR == 0 { break; }
        else if trials == 0 { return Err(Timeout) }
    
    }

    let data_rx = esp_loader_read_register(SPI_W0_REG)?;

    // Restore SPI configuration
    esp_loader_write_register(SPI_USR_REG, old_spi_usr)?;
    esp_loader_write_register(SPI_USR2_REG, old_spi_usr2)?;

    Ok(data_rx)
}

fn detect_flash_size() -> Result<u32, EspLoaderError>
{
    let flash_id = spi_flash_command(SpiFlashReadId, None, 24)?;
    let size_id = (flash_id >> 16) as usize;

    if size_id < 0x12 || size_id > 0x18 {
        return Err(UnsupportedChip)
    }

    Ok(SIZE_ID_TO_FLASH_SIZE[size_id - 0x12])
}

fn init_md5(_address: u32, _size: u32)
{

}

fn md5_update(_data: &[u8], _size: usize)
{

}

fn md5_final() -> [u8; 16]
{
    let ret: [u8; 16] = [0; 16];
    ret 
}

fn esp_loader_flash_start(offset: u32, image_size: u32, block_size: u32) -> Result<(), EspLoaderError>
{
    let blocks_to_write = (image_size + block_size - 1) / block_size;
    let erase_size = block_size * blocks_to_write;
    unsafe { S_FLASH_WRITE_SIZE = block_size as usize; }
    
    match detect_flash_size() {
        Ok(flash_size) => {
            if image_size > flash_size { return Err(ImageSize); }
            start_timer_default();
            loader_spi_parameters(flash_size)?;
        }
        _ => {
            loader_port_debug_print("Flash size detection failed, falling back to default");
        }
    }

    init_md5(offset, image_size);

    loader_port_start_timer(timeout_per_mb(erase_size, ERASE_REGION_TIMEOUT_PER_MB));
    loader_flash_begin_cmd(offset, erase_size, block_size, blocks_to_write)
}

fn esp_loader_flash_write(payload: &[u8]) -> Result<(), EspLoaderError>
{
    md5_update(payload, (payload.len()+ 3) & !3);
    start_timer_default();
    loader_flash_data_cmd(&payload)?;

    let mut padding_bytes = unsafe { S_FLASH_WRITE_SIZE - payload.len() };

    while padding_bytes != 0 {
        let padding: [u8; 32] = [PADDING_PATTERN; 32];
        let remaining = min(padding_bytes, padding.len());

        md5_update(&padding, (remaining + 3) & !3);
        start_timer_default();
        loader_flash_data_cmd(&padding[0..remaining])?;
        padding_bytes -= remaining;
    }
    Ok(())
}

fn esp_loader_flash_finish(reboot: bool) -> Result<(), EspLoaderError>
{
    start_timer_default();
    loader_flash_end_cmd(!reboot)
}

fn esp_loader_read_register(address: u32) -> Result<u32, EspLoaderError>
{
    start_timer_default();
    loader_read_reg_cmd(address)
}


fn esp_loader_write_register(address: u32, value: u32) -> Result<(), EspLoaderError>
{
    start_timer_default();
    loader_write_reg_cmd(address, value, 0xFFFFFFFF, 0)
}

// TODO #ifndef TARGET_ESP8266
fn esp_loader_change_baudrate(baudrate: u32) -> Result<(), EspLoaderError>
{
    start_timer_default();
    loader_change_baudrate_cmd(baudrate)
}

fn hexify(raw_md5: &[u8; 16]) -> [u8; 32]
{
    let mut hex_md5: [u8; 32] = [0; 32];

    const DEC_TO_HEX: [char; 16] = [
        '0', '1', '2', '3', '4', '5', '6', '7',
        '8', '9', 'a', 'b', 'c', 'd', 'e', 'f'
    ];

    for (i, &elem) in raw_md5.iter().enumerate() {
        let high_nibble = elem / 16;
        let low_nibble = elem - (high_nibble * 16);
        hex_md5[i] = DEC_TO_HEX[high_nibble as usize] as u8;
        hex_md5[i+1] = DEC_TO_HEX[low_nibble as usize] as u8;
    }

    hex_md5
}

static mut S_IMAGE_SIZE: u32 = 0;
static mut S_START_ADDRESS: u32 = 0;

use std::str::from_utf8_unchecked;

fn print_md5_debug(computed_md5: &[u8], received_md5: &[u8])
{
    loader_port_debug_print("Error: MD5 checksum does not match:");
    loader_port_debug_print("Expected:");
    unsafe { loader_port_debug_print(from_utf8_unchecked(&received_md5)) };
    loader_port_debug_print("Actual:");
    unsafe { loader_port_debug_print(from_utf8_unchecked(&computed_md5)) };
}

// TODO #ifndef TARGET_ESP8266
fn esp_loader_flash_verify() -> Result<(), EspLoaderError>
{
    let start_address = unsafe { S_START_ADDRESS };
    let image_size = unsafe { S_IMAGE_SIZE };

    loader_port_start_timer(timeout_per_mb(start_address, MD5_TIMEOUT_PER_MB));
    let received_md5 = loader_md5_cmd(start_address, image_size)?;
    let computed_md5 = hexify( &md5_final() );

    if computed_md5 != received_md5 {
        print_md5_debug(&computed_md5, &received_md5);
        return Err(InvalidMD5)
    }

    Ok(())
}

fn esp_loader_reset_target()
{
    loader_port_reset_target();
}

fn main() {

    match detect_chip() {
        Ok(()) => println!("Ok"),
        Err(err) => println!("Error {}", err as u8),
    }
}