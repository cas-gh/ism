#![windows_subsystem = "windows"]

use eframe::egui;
use ping::ping;
use std::fs::File;
use std::io::Write;
use std::net::ToSocketAddrs;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use chrono::Local;


// Struct to hold the state of the Internet Monitor application
struct InternetMonitor {
    is_monitoring: bool, // Flag indicating if monitoring is active
    start_time: Option<Instant>, // The time when monitoring started
    last_check: Option<Instant>, // The time of the last check
    last_frame_time: Instant, // Time of the last frame for FPS control
    status: Arc<Mutex<String>>, // Current status message
    response_times: Arc<Mutex<Vec<(f64, f64)>>>, // List of response times (elapsed time, response time)
    log_status: Arc<Mutex<Option<String>>>, // Status message for logging
    total_data_sent: Arc<Mutex<u64>>, // Total data sent in bytes
    longest_response_time: Arc<Mutex<f64>>, // Longest response time recorded
    last_log_file_name: Arc<Mutex<Option<String>>>, // Track the last log file name
    last_log_time: Arc<Mutex<Option<Instant>>>,
}

impl Default for InternetMonitor {
    fn default() -> Self {
        Self {
            is_monitoring: false,
            start_time: None,
            last_check: None,
            last_frame_time: Instant::now(),
            status: Arc::new(Mutex::new("Not checked yet".to_string())),
            response_times: Arc::new(Mutex::new(Vec::new())),
            log_status: Arc::new(Mutex::new(None)),
            total_data_sent: Arc::new(Mutex::new(0)),
            longest_response_time: Arc::new(Mutex::new(0.0)),
            last_log_file_name: Arc::new(Mutex::new(None)),
            last_log_time: Arc::new(Mutex::new(None)),
        }
    }
}

impl eframe::App for InternetMonitor {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Calculate time since the last frame
        let frame_duration = Instant::now().duration_since(self.last_frame_time);
        let target_frame_duration = Duration::from_secs_f64(1.0 / 60.0); // 60 FPS

        // If the last frame was rendered too quickly, sleep to maintain 60 FPS
        if frame_duration < target_frame_duration {
            thread::sleep(target_frame_duration - frame_duration);
        }

        // Update the last frame time to now
        self.last_frame_time = Instant::now();

        // UI code
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Internet Stability Monitor");

            // Create a horizontal layout for buttons
            ui.horizontal(|ui| {
                // Button to start/stop monitoring
                if ui.button(if self.is_monitoring { "Stop Monitoring" } else { "Start Monitoring" }).clicked() {
                    self.is_monitoring = !self.is_monitoring;
                    let mut status = self.status.lock().unwrap();
                    if self.is_monitoring {
                        *status = format!("Monitoring {}...", "google.com");
                        self.start_time = Some(Instant::now());
                        self.last_check = Some(Instant::now());
                    } else {
                        *status = "Not monitoring".to_string();
                    }
                }

                // Button to clear data and stop monitoring
                if ui.button("Clear Data").clicked() {
                    // Stop monitoring
                    self.is_monitoring = false;
                    let mut status = self.status.lock().unwrap();
                    *status = "Not monitoring".to_string();
                    
                    // Clear data
                    let mut data = self.response_times.lock().unwrap();
                    data.clear();
                    let mut total_data_sent = self.total_data_sent.lock().unwrap();
                    *total_data_sent = 0; // Reset total data sent to 0

                    // Update log status message to indicate data has been cleared
                    let mut log_status = self.log_status.lock().unwrap();
                    *log_status = Some("Data cleared".to_string());

                    // Reset log status message after a short delay
                    let log_status_clone = Arc::clone(&self.log_status);
                    thread::spawn(move || {
                        thread::sleep(Duration::from_secs(2)); // Wait for 2 seconds
                        let mut log_status = log_status_clone.lock().unwrap();
                        *log_status = None; // Clear the status message
                    });
                }

                // Button to log data and display log status message
                if ui.button("Log Data").clicked() {
                    self.log_data();
                }

                // Display the log status message next to the log button
                let log_status = self.log_status.lock().unwrap();
                if let Some(message) = log_status.as_ref() {
                    ui.label(message);
                }
            });

            // Display the current status with conditional color
            let status = self.status.lock().unwrap();
            let text_color = if self.is_monitoring {
                egui::Color32::from_rgb(144, 238, 144) // Light green color
            } else {
                egui::Color32::WHITE // Default color
            };
            ui.label(egui::RichText::new(format!("Status: {}", *status)).color(text_color));

            // Display elapsed time and response time on separate lines
            let data = self.response_times.lock().unwrap();
            if let Some((elapsed_time, response_time)) = data.last() {
                ui.label(format!("Elapsed Time: {:.0} s", elapsed_time));
                ui.label(format!("Response Time: {:.0} ms", response_time));
            } else {
                ui.label("No data available.");
            }

            // Calculate and display average response time
            if !data.is_empty() {
                let average_response_time = data.iter().map(|(_, response_time)| response_time).sum::<f64>() / data.len() as f64;
                ui.label(format!("Average Response Time: {:.0} ms", average_response_time));

                // Display longest response time
                if let Some(max_response_time) = data.iter().map(|(_, response_time)| response_time).max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)) {
                    ui.label(format!("Longest Response Time: {:.0} ms", max_response_time));
                }
            }

            // Display total data sent
            let total_data_sent = self.total_data_sent.lock().unwrap();
            ui.label(format!("Total Data Sent: {} bytes", total_data_sent));

            // Check if monitoring is active
            if self.is_monitoring {
                if let Some(last_check) = self.last_check {
                    // Check every second
                    if last_check.elapsed() >= Duration::from_secs(1) {
                        self.check_connection();
                        self.last_check = Some(Instant::now());
                    }
                }
            }
        });

        // Request a redraw to keep updating the UI
        ctx.request_repaint();
    }
}

impl InternetMonitor {
    // Function to check the internet connection
    fn check_connection(&self) {
        let status_clone = Arc::clone(&self.status);
        let response_times_clone = Arc::clone(&self.response_times);
        let start_time_clone = self.start_time.clone();
        let total_data_sent_clone = Arc::clone(&self.total_data_sent);
        let longest_response_time_clone = Arc::clone(&self.longest_response_time);
        let last_log_file_name_clone = Arc::clone(&self.last_log_file_name);
        let last_log_time_clone = Arc::clone(&self.last_log_time);
    
        thread::spawn(move || {
            let target = "google.com";
            let target_ip = match (target, 80).to_socket_addrs() {
                Ok(mut addrs) => addrs.next().map(|addr| addr.ip()),
                Err(_e) => None,
            };
    
            let target_ip = match target_ip {
                Some(ip) => ip,
                None => return,
            };
    
            let start_time = Instant::now();
    
            // Perform the ping
            let result = ping(target_ip, None, Some(32), None, None, None);
    
            let response_time_ms = match result {
                Ok(_) => start_time.elapsed().as_millis() as f64,
                Err(_) => 0.0,
            };
    
            let status_message = if response_time_ms > 0.0 {
                format!("Connected to {}.", target)
            } else {
                format!("Disconnected from {}.", target)
            };
    
            let mut status = status_clone.lock().unwrap();
            *status = status_message;
    
            let elapsed_since_start = if let Some(start_time) = start_time_clone {
                Instant::now().duration_since(start_time).as_secs_f64()
            } else {
                0.0
            };
    
            let mut data = response_times_clone.lock().unwrap();
            data.push((elapsed_since_start, response_time_ms));
    
            // Update longest response time
            let mut longest_response_time = longest_response_time_clone.lock().unwrap();
            if response_time_ms > *longest_response_time {
                *longest_response_time = response_time_ms;
    
                // Check if we need to create a new log file
                if response_time_ms > 175.0 {
                    // Check if it's been at least a minute since the last log
                    let mut last_log_time = last_log_time_clone.lock().unwrap();
                    if last_log_time.map_or(true, |t| t.elapsed() >= Duration::from_secs(60)) {
                        let timestamp = Local::now().format("%Y%m%d%H%M%S").to_string();
                        let new_log_file_name = format!("log_{}.txt", timestamp);
    
                        let mut last_log_file_name = last_log_file_name_clone.lock().unwrap();
                        *last_log_file_name = Some(new_log_file_name.clone());
    
                        // Log data to the new file
                        let data_clone = Arc::clone(&response_times_clone);
                        let total_data_sent_clone = Arc::clone(&total_data_sent_clone);
                        let longest_response_time_clone = Arc::clone(&longest_response_time_clone);
    
                        thread::spawn(move || {
                            let data = data_clone.lock().unwrap();
                            let average_response_time = if !data.is_empty() {
                                data.iter().map(|(_, response_time)| response_time).sum::<f64>() / data.len() as f64
                            } else {
                                0.0
                            };
    
                            let total_data_sent = *total_data_sent_clone.lock().unwrap();
                            let longest_response_time = *longest_response_time_clone.lock().unwrap();
    
                            let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    
                            let log_content = if !data.is_empty() {
                                let mut log_content = format!(
                                    "Log Created: {}\nPing Target: {}\nAverage Response Time: {:.0} ms\nLongest Response Time: {:.0} ms\nTotal Data Sent: {} bytes\n\n",
                                    timestamp,
                                    target,
                                    average_response_time,
                                    longest_response_time,
                                    total_data_sent
                                );
    
                                for (elapsed_time, response_time) in data.iter() {
                                    let rounded_elapsed_time = elapsed_time.round();
                                    log_content.push_str(&format!(
                                        "{:.0} s, {:.0} ms\n",
                                        rounded_elapsed_time, response_time
                                    ));
                                }
    
                                log_content
                            } else {
                                format!("Log Created: {}\nNo data to log.", timestamp)
                            };
    
                            let mut file = match File::create(&new_log_file_name) {
                                Ok(file) => file,
                                Err(_e) => return,
                            };
    
                            if let Err(_e) = writeln!(file, "{}", log_content) {
                                return;
                            }
                        });
    
                        // Update the last log time
                        *last_log_time = Some(Instant::now());
                    }
                }
            }
    
            let mut total_data_sent = total_data_sent_clone.lock().unwrap();
            *total_data_sent += 32;
    
            // Keep the data size manageable
            if data.len() > 100 {
                data.remove(0);
            }
    
            // Avoid busy-waiting
            thread::sleep(Duration::from_millis(100));
        });
    }    

    // Function to log data to a file
    fn log_data(&self) {
        let start_time = self.start_time.clone().unwrap_or_else(Instant::now);
        let end_time = Instant::now() - Duration::from_secs(100); 

        let data = self.response_times.lock().unwrap();
        let filtered_data: Vec<(f64, f64)> = data
            .iter()
            .filter(|(elapsed_time, _)| {
                let elapsed_since_start = Duration::from_secs_f64(*elapsed_time);
                elapsed_since_start > end_time.duration_since(start_time)
            })
            .cloned()
            .collect();

        let average_response_time = if !data.is_empty() {
            data.iter().map(|(_, response_time)| response_time).sum::<f64>() / data.len() as f64
        } else {
            0.0
        };

        let total_data_sent = *self.total_data_sent.lock().unwrap();
        let longest_response_time = *self.longest_response_time.lock().unwrap();

        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

        let log_content = if !filtered_data.is_empty() {
            let mut log_content = format!(
                "Log Created: {}\nPing Target: {}\nAverage Response Time: {:.0} ms\nLongest Response Time: {:.0} ms\nTotal Data Sent: {} bytes\n\n",
                timestamp,
                "google.com",
                average_response_time,
                longest_response_time,
                total_data_sent
            );

            for (elapsed_time, response_time) in filtered_data {
                let rounded_elapsed_time = elapsed_time.round();
                log_content.push_str(&format!(
                    "{:.0} s, {:.0} ms\n",
                    rounded_elapsed_time, response_time
                ));
            }

            log_content
        } else {
            format!("Log Created: {}\nNo data to log.", timestamp)
        };

        let file_name = "log.txt";
        let mut file = match File::create(file_name) {
            Ok(file) => file,
            Err(_e) => return,
        };

        if let Err(_e) = writeln!(file, "{}", log_content) {
            return;
        }

        let mut log_status = self.log_status.lock().unwrap();
        *log_status = Some("✔".to_string());

        // Reset log status message after a short delay
        let log_status_clone = Arc::clone(&self.log_status);
        thread::spawn(move || {
            thread::sleep(Duration::from_secs(2));
            let mut log_status = log_status_clone.lock().unwrap();
            *log_status = None;
        });
    }
}

fn main() {
    let options = eframe::NativeOptions::default();
    let _ = eframe::run_native(
        "Internet Stability Monitor",
        options,
        Box::new(|_cc| Ok(Box::new(InternetMonitor::default()))),
    );
}
