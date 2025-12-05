use esp_idf_svc::hal::{
    delay::FreeRtos,
    gpio::{Gpio4, PinDriver},
    i2c::{I2cConfig, I2cDriver},
    peripherals::Peripherals,
    prelude::*,
};
use esp_idf_svc::sys as esp_idf_sys;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

// Configurações
const INTERVALO_LEITURA_MINUTOS: u64 = 10;
const ARQUIVO_BMP280: &str = "/spiffs/bmp280_data.txt";
const ARQUIVO_DHT11: &str = "/spiffs/dht11_data.txt";
const BMP280_ADDR: u8 = 0x76;

// ============================================
// Estruturas de Configuração
// ============================================

#[derive(Clone)]
struct Config {
    intervalo_minutos: u64,
}

impl Config {
    fn new() -> Self {
        Self {
            intervalo_minutos: INTERVALO_LEITURA_MINUTOS,
        }
    }

    fn set_intervalo(&mut self, minutos: u64) {
        self.intervalo_minutos = minutos;
    }

    fn intervalo_ms(&self) -> u64 {
        self.intervalo_minutos * 60 * 1000
    }
}

#[derive(Debug)]
struct DadosBMP280 {
    temperatura: f32,
    pressao: f32,
    altitude: f32,
}

#[derive(Debug)]
struct DadosDHT11 {
    temperatura: f32,
    umidade: f32,
}

// ============================================
// Driver BMP280 com Calibração Completa
// ============================================

#[derive(Debug)]
struct CalibracaoBMP280 {
    dig_t1: u16,
    dig_t2: i16,
    dig_t3: i16,
    dig_p1: u16,
    dig_p2: i16,
    dig_p3: i16,
    dig_p4: i16,
    dig_p5: i16,
    dig_p6: i16,
    dig_p7: i16,
    dig_p8: i16,
    dig_p9: i16,
}

struct BMP280<'a> {
    i2c: Arc<Mutex<I2cDriver<'a>>>,
    addr: u8,
    calibracao: CalibracaoBMP280,
    t_fine: i32,
}

impl<'a> BMP280<'a> {
    fn new(i2c: Arc<Mutex<I2cDriver<'a>>>, addr: u8) -> Result<Self, esp_idf_sys::EspError> {
        let mut sensor = Self {
            i2c,
            addr,
            calibracao: CalibracaoBMP280 {
                dig_t1: 0,
                dig_t2: 0,
                dig_t3: 0,
                dig_p1: 0,
                dig_p2: 0,
                dig_p3: 0,
                dig_p4: 0,
                dig_p5: 0,
                dig_p6: 0,
                dig_p7: 0,
                dig_p8: 0,
                dig_p9: 0,
            },
            t_fine: 0,
        };

        // Verificar chip ID
        let mut chip_id = [0u8; 1];
        sensor.read_register(0xD0, &mut chip_id)?;

        if chip_id[0] != 0x58 {
            println!(
                "Aviso: Chip ID inesperado: 0x{:02X} (esperado 0x58)",
                chip_id[0]
            );
        }

        // Ler coeficientes de calibração
        sensor.ler_calibracao()?;

        // Resetar sensor
        sensor.write_register(0xE0, 0xB6)?;
        FreeRtos::delay_ms(10);

        // Configurar sensor
        sensor.init()?;

        Ok(sensor)
    }

    fn ler_calibracao(&mut self) -> Result<(), esp_idf_sys::EspError> {
        let mut calib = [0u8; 24];
        self.read_register(0x88, &mut calib)?;

        self.calibracao.dig_t1 = u16::from_le_bytes([calib[0], calib[1]]);
        self.calibracao.dig_t2 = i16::from_le_bytes([calib[2], calib[3]]);
        self.calibracao.dig_t3 = i16::from_le_bytes([calib[4], calib[5]]);

        self.calibracao.dig_p1 = u16::from_le_bytes([calib[6], calib[7]]);
        self.calibracao.dig_p2 = i16::from_le_bytes([calib[8], calib[9]]);
        self.calibracao.dig_p3 = i16::from_le_bytes([calib[10], calib[11]]);
        self.calibracao.dig_p4 = i16::from_le_bytes([calib[12], calib[13]]);
        self.calibracao.dig_p5 = i16::from_le_bytes([calib[14], calib[15]]);
        self.calibracao.dig_p6 = i16::from_le_bytes([calib[16], calib[17]]);
        self.calibracao.dig_p7 = i16::from_le_bytes([calib[18], calib[19]]);
        self.calibracao.dig_p8 = i16::from_le_bytes([calib[20], calib[21]]);
        self.calibracao.dig_p9 = i16::from_le_bytes([calib[22], calib[23]]);

        println!("Calibração BMP280 carregada:");
        println!(
            "  T1={}, T2={}, T3={}",
            self.calibracao.dig_t1, self.calibracao.dig_t2, self.calibracao.dig_t3
        );
        println!(
            "  P1={}, P2={}, P3={}",
            self.calibracao.dig_p1, self.calibracao.dig_p2, self.calibracao.dig_p3
        );

        Ok(())
    }

    fn init(&self) -> Result<(), esp_idf_sys::EspError> {
        // Configurar modo normal, oversampling x16 para temp e pressão
        // osrs_t[7:5] = 101 (x16), osrs_p[4:2] = 101 (x16), mode[1:0] = 11 (normal)
        self.write_register(0xF4, 0b10110111)?;

        // Configurar standby time = 0.5ms, filter = 16
        // t_sb[7:5] = 000, filter[4:2] = 100, spi3w_en[0] = 0
        self.write_register(0xF5, 0b00010000)?;

        FreeRtos::delay_ms(100);
        Ok(())
    }

    fn write_register(&self, reg: u8, value: u8) -> Result<(), esp_idf_sys::EspError> {
        let mut i2c = self.i2c.lock().unwrap();
        i2c.write(self.addr, &[reg, value], 1000)
    }

    fn read_register(&self, reg: u8, buffer: &mut [u8]) -> Result<(), esp_idf_sys::EspError> {
        let mut i2c = self.i2c.lock().unwrap();
        i2c.write_read(self.addr, &[reg], buffer, 1000)
    }

    fn compensar_temperatura(&mut self, adc_t: i32) -> f32 {
        let var1 = (((adc_t >> 3) - ((self.calibracao.dig_t1 as i32) << 1))
            * (self.calibracao.dig_t2 as i32))
            >> 11;

        let var2 = (((((adc_t >> 4) - (self.calibracao.dig_t1 as i32))
            * ((adc_t >> 4) - (self.calibracao.dig_t1 as i32)))
            >> 12)
            * (self.calibracao.dig_t3 as i32))
            >> 14;

        self.t_fine = var1 + var2;

        let t = (self.t_fine * 5 + 128) >> 8;
        t as f32 / 100.0
    }

    fn compensar_pressao(&self, adc_p: i32) -> f32 {
        let mut var1: i64 = (self.t_fine as i64) - 128000;
        let mut var2: i64 = var1 * var1 * (self.calibracao.dig_p6 as i64);

        var2 = var2 + ((var1 * (self.calibracao.dig_p5 as i64)) << 17);
        var2 = var2 + ((self.calibracao.dig_p4 as i64) << 35);
        var1 = ((var1 * var1 * (self.calibracao.dig_p3 as i64)) >> 8)
            + ((var1 * (self.calibracao.dig_p2 as i64)) << 12);
        var1 = ((((1i64) << 47) + var1) * (self.calibracao.dig_p1 as i64)) >> 33;

        if var1 == 0 {
            return 0.0;
        }

        let mut p: i64 = 1048576 - adc_p as i64;
        p = (((p << 31) - var2) * 3125) / var1;
        var1 = ((self.calibracao.dig_p9 as i64) * (p >> 13) * (p >> 13)) >> 25;
        var2 = ((self.calibracao.dig_p8 as i64) * p) >> 19;
        p = ((p + var1 + var2) >> 8) + ((self.calibracao.dig_p7 as i64) << 4);

        (p as f32) / 256.0
    }

    fn calcular_altitude(&self, pressao_hpa: f32) -> f32 {
        44330.0 * (1.0 - (pressao_hpa / 1013.25_f32).powf(0.1903))
    }

    fn ler_dados(&mut self) -> Result<DadosBMP280, esp_idf_sys::EspError> {
        // Aguardar medição estar pronta
        let mut status = [0u8; 1];
        for _ in 0..10 {
            self.read_register(0xF3, &mut status)?;
            if (status[0] & 0x08) == 0 {
                break;
            }
            FreeRtos::delay_ms(10);
        }

        // Ler dados raw (burst read de 0xF7 a 0xFC)
        let mut buffer = [0u8; 6];
        self.read_register(0xF7, &mut buffer)?;

        let adc_p =
            ((buffer[0] as i32) << 12) | ((buffer[1] as i32) << 4) | ((buffer[2] as i32) >> 4);
        let adc_t =
            ((buffer[3] as i32) << 12) | ((buffer[4] as i32) << 4) | ((buffer[5] as i32) >> 4);

        // Compensar temperatura (atualiza t_fine)
        let temperatura = self.compensar_temperatura(adc_t);

        // Compensar pressão (usa t_fine)
        let pressao_pa = self.compensar_pressao(adc_p);
        let pressao_hpa = pressao_pa / 100.0;

        // Calcular altitude
        let altitude = self.calcular_altitude(pressao_hpa);

        Ok(DadosBMP280 {
            temperatura,
            pressao: pressao_hpa,
            altitude,
        })
    }
}

// ============================================
// Driver DHT11 Completo
// ============================================

struct DHT11<'a> {
    pin: PinDriver<'a, Gpio4, esp_idf_svc::hal::gpio::InputOutput>,
}

impl<'a> DHT11<'a> {
    fn new(pin: Gpio4) -> Result<Self, esp_idf_sys::EspError> {
        let pin = PinDriver::input_output_od(pin)?;
        Ok(Self { pin })
    }

    fn esperar_nivel(
        &mut self,
        nivel: bool,
        timeout_us: u32,
    ) -> Result<u32, esp_idf_sys::EspError> {
        let start = esp_idf_sys::esp_timer_get_time();

        while self.pin.is_high() != nivel {
            if (esp_idf_sys::esp_timer_get_time() - start) > timeout_us as i64 {
                return Err(esp_idf_sys::EspError::from_infallible::<
                    { esp_idf_sys::ESP_ERR_TIMEOUT },
                >());
            }
        }

        Ok((esp_idf_sys::esp_timer_get_time() - start) as u32)
    }

    fn ler_bit(&mut self) -> Result<bool, esp_idf_sys::EspError> {
        // Esperar sinal baixo (início do bit)
        self.esperar_nivel(false, 100)?;

        // Esperar sinal alto
        self.esperar_nivel(true, 100)?;

        // Medir duração do sinal alto
        let duracao = self.esperar_nivel(false, 100)?;

        // Se duração > ~40us, é bit 1, senão é bit 0
        Ok(duracao > 40)
    }

    fn ler_byte(&mut self) -> Result<u8, esp_idf_sys::EspError> {
        let mut byte: u8 = 0;

        for i in 0..8 {
            if self.ler_bit()? {
                byte |= 1 << (7 - i);
            }
        }

        Ok(byte)
    }

    fn ler_dados(&mut self) -> Result<DadosDHT11, esp_idf_sys::EspError> {
        // Desabilitar interrupções para timing preciso
        unsafe {
            esp_idf_sys::portDISABLE_INTERRUPTS();
        }

        // 1. Enviar sinal de início
        self.pin.set_high()?;
        FreeRtos::delay_ms(1);

        self.pin.set_low()?;
        esp_idf_svc::hal::delay::Ets::delay_us(18000); // 18ms

        self.pin.set_high()?;
        esp_idf_svc::hal::delay::Ets::delay_us(40);

        // 2. Aguardar resposta do DHT11
        // DHT puxa baixo por 80us
        if let Err(_) = self.esperar_nivel(false, 100) {
            unsafe {
                esp_idf_sys::portENABLE_INTERRUPTS();
            }
            println!("DHT11: Timeout esperando resposta (baixo)");
            return Err(esp_idf_sys::EspError::from_infallible::<
                { esp_idf_sys::ESP_ERR_TIMEOUT },
            >());
        }

        // DHT puxa alto por 80us
        if let Err(_) = self.esperar_nivel(true, 100) {
            unsafe {
                esp_idf_sys::portENABLE_INTERRUPTS();
            }
            println!("DHT11: Timeout esperando resposta (alto)");
            return Err(esp_idf_sys::EspError::from_infallible::<
                { esp_idf_sys::ESP_ERR_TIMEOUT },
            >());
        }

        // 3. Ler 40 bits de dados (5 bytes)
        let resultado = (|| -> Result<[u8; 5], esp_idf_sys::EspError> {
            let mut dados = [0u8; 5];

            for i in 0..5 {
                dados[i] = self.ler_byte()?;
            }

            Ok(dados)
        })();

        // Reabilitar interrupções
        unsafe {
            esp_idf_sys::portENABLE_INTERRUPTS();
        }

        let dados = resultado?;

        // 4. Verificar checksum
        let checksum = dados[0]
            .wrapping_add(dados[1])
            .wrapping_add(dados[2])
            .wrapping_add(dados[3]);

        if checksum != dados[4] {
            println!(
                "DHT11: Checksum inválido! Calculado: {}, Recebido: {}",
                checksum, dados[4]
            );
            return Err(esp_idf_sys::EspError::from_infallible::<
                { esp_idf_sys::ESP_ERR_INVALID_CRC },
            >());
        }

        // 5. Converter dados
        let umidade = dados[0] as f32 + (dados[1] as f32) * 0.1;
        let temperatura = dados[2] as f32 + (dados[3] as f32) * 0.1;

        Ok(DadosDHT11 {
            temperatura,
            umidade,
        })
    }
}
// ============================================
// Funções de Gravação
// ============================================

fn gravar_bmp280(dados: &DadosBMP280) -> std::io::Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(ARQUIVO_BMP280)?;

    let timestamp = esp_idf_sys::esp_timer_get_time() / 1000000;
    let linha = format!(
        "{},{:.2},{:.2},{:.2}\n",
        timestamp, dados.temperatura, dados.pressao, dados.altitude
    );

    file.write_all(linha.as_bytes())?;
    file.flush()?;

    println!(
        "✓ BMP280: T={:.2}°C, P={:.2}hPa, Alt={:.2}m",
        dados.temperatura, dados.pressao, dados.altitude
    );

    Ok(())
}

fn gravar_dht11(dados: &DadosDHT11) -> std::io::Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(ARQUIVO_DHT11)?;

    let timestamp = esp_idf_sys::esp_timer_get_time() / 1000000;
    let linha = format!(
        "{},{:.2},{:.2}\n",
        timestamp, dados.temperatura, dados.umidade
    );

    file.write_all(linha.as_bytes())?;
    file.flush()?;

    println!(
        "✓ DHT11: T={:.2}°C, RH={:.2}%",
        dados.temperatura, dados.umidade
    );

    Ok(())
}

// ============================================
// Tasks Assíncronas
// ============================================

fn task_bmp280(config: Arc<Mutex<Config>>, i2c: Arc<Mutex<I2cDriver<'static>>>) {
    println!("🚀 Task BMP280 iniciada");

    let mut sensor = match BMP280::new(i2c, BMP280_ADDR) {
        Ok(s) => s,
        Err(e) => {
            println!("❌ Erro ao inicializar BMP280: {:?}", e);
            return;
        }
    };

    let mut contador_erros = 0;
    const MAX_ERROS: u32 = 5;

    loop {
        match sensor.ler_dados() {
            Ok(dados) => {
                if let Err(e) = gravar_bmp280(&dados) {
                    println!("⚠️  Erro ao gravar BMP280: {:?}", e);
                }
                contador_erros = 0;
            }
            Err(e) => {
                contador_erros += 1;
                println!(
                    "⚠️  Erro ao ler BMP280 ({}/{}): {:?}",
                    contador_erros, MAX_ERROS, e
                );

                if contador_erros >= MAX_ERROS {
                    println!("❌ BMP280: Muitos erros consecutivos, reiniciando sensor...");
                    FreeRtos::delay_ms(1000);
                    // Tentar reinicializar
                    match BMP280::new(Arc::clone(&sensor.i2c), BMP280_ADDR) {
                        Ok(s) => {
                            sensor = s;
                            contador_erros = 0;
                            println!("✓ BMP280 reinicializado");
                        }
                        Err(e) => {
                            println!("❌ Falha ao reinicializar BMP280: {:?}", e);
                        }
                    }
                }
            }
        }

        let intervalo = config.lock().unwrap().intervalo_ms();
        thread::sleep(Duration::from_millis(intervalo));
    }
}

fn task_dht11(config: Arc<Mutex<Config>>, gpio4: Gpio4) {
    println!("🚀 Task DHT11 iniciada");

    let mut sensor = match DHT11::new(gpio4) {
        Ok(s) => s,
        Err(e) => {
            println!("❌ Erro ao inicializar DHT11: {:?}", e);
            return;
        }
    };

    let mut contador_erros = 0;
    const MAX_ERROS: u32 = 5;

    loop {
        match sensor.ler_dados() {
            Ok(dados) => {
                if let Err(e) = gravar_dht11(&dados) {
                    println!("⚠️  Erro ao gravar DHT11: {:?}", e);
                }
                contador_erros = 0;
            }
            Err(e) => {
                contador_erros += 1;
                println!(
                    "⚠️  Erro ao ler DHT11 ({}/{}): {:?}",
                    contador_erros, MAX_ERROS, e
                );

                if contador_erros >= MAX_ERROS {
                    println!("❌ DHT11: Muitos erros consecutivos");
                    contador_erros = 0;
                }
            }
        }

        let intervalo = config.lock().unwrap().intervalo_ms();
        thread::sleep(Duration::from_millis(intervalo));
    }
}

// ============================================
// Main
// ============================================

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    println!("\n╔════════════════════════════════════════╗");
    println!("║  Sistema de Leitura de Sensores       ║");
    println!("║  ESP32-S3 + BMP280 + DHT11            ║");
    println!("╚════════════════════════════════════════╝\n");

    let config = Arc::new(Mutex::new(Config::new()));
    let peripherals = Peripherals::take()?;

    // Configurar I2C para BMP280
    println!("⚙️  Configurando I2C...");
    let i2c_config = I2cConfig::new()
        .baudrate(100.kHz().into())
        .sda_enable_pullup(true)
        .scl_enable_pullup(true);

    let i2c = I2cDriver::new(
        peripherals.i2c0,
        peripherals.pins.gpio21,
        peripherals.pins.gpio22,
        &i2c_config,
    )?;

    let i2c = Arc::new(Mutex::new(i2c));

    println!("⚙️  Configurando GPIO para DHT11...");
    let gpio4 = peripherals.pins.gpio4;

    // Criar threads
    let config_bmp = Arc::clone(&config);
    let i2c_bmp = Arc::clone(&i2c);

    let handle_bmp = thread::Builder::new()
        .stack_size(8192)
        .name("bmp280".to_string())
        .spawn(move || task_bmp280(config_bmp, i2c_bmp))?;

    let config_dht = Arc::clone(&config);
    let handle_dht = thread::Builder::new()
        .stack_size(8192)
        .name("dht11".to_string())
        .spawn(move || task_dht11(config_dht, gpio4))?;

    println!("\n✓ Sistema iniciado!");
    println!(
        "📊 Intervalo de leitura: {} minutos",
        config.lock().unwrap().intervalo_minutos
    );
    println!("📁 Arquivos de dados:");
    println!("   - {}", ARQUIVO_BMP280);
    println!("   - {}\n", ARQUIVO_DHT11);

    // Aguardar threads
    handle_bmp.join().unwrap();
    handle_dht.join().unwrap();

    Ok(())
}
