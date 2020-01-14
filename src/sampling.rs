extern crate reqwest;

use crate::thingworxjson::*;
use crate::thingworxtestconfig::{OneTimeTest, RepeatTest, ThingworxServer};
use chrono::offset::Utc;
use chrono::DateTime;
use influx_db_client::{Point, Value};
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use reqwest::Client as ReqClient;
use std::collections::{BTreeMap, HashMap};
use std::error::Error;
use std::fs::OpenOptions;
use std::io::LineWriter;
use std::io::Write;
use std::path::Path;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};
use sys_info::*;

pub fn samping_one_time(testid: &str, o_sampling: &OneTimeTest) -> Result<Point, Box<dyn Error>> {
    //meansurement name
    let mut point = Point::new("OS_Status");

    point.add_tag("testid", Value::String(testid.to_string()));
    if o_sampling.os_type {
        point.add_field(
            "os_type",
            Value::String(match os_type() {
                Ok(value) => value,
                Err(_) => "Unknown".to_string(),
            }),
        );
    }
    if o_sampling.cpu_num {
        point.add_field(
            "cpu_num",
            Value::Integer(match cpu_num() {
                Ok(value) => value as i64,
                Err(_) => 0 as i64,
            }),
        );
    }
    //.add_field("proc_total",Value::String(match proc_total(){ Ok(value)=>value,Err(_)=>"Unknown",}))
    if o_sampling.cpu_speed {
        point.add_field(
            "cpu_speed",
            Value::Integer(match cpu_speed() {
                Ok(value) => value as i64,
                Err(_) => 0 as i64,
            }),
        );
    }
    if o_sampling.hostname {
        point.add_field(
            "hostname",
            Value::String(match hostname() {
                Ok(value) => value,
                Err(_) => "Unknown".to_string(),
            }),
        );
    }

    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?;

    point.add_timestamp(timestamp.as_millis() as i64);
    debug!("{:?}", point);
    Ok(point)
}

pub fn sampling_repeat(
    testid: &str,
    r_sampling: &RepeatTest,
    export_path: &Path,
    export_file: bool,
) -> Result<Point, Box<dyn Error>> {
    let mut point = Point::new("OS_Sampling");
    point.add_tag("testid", Value::String(testid.to_string()));

    //let r_sampling = &test_config.testmachine.repeat_sampling;

    let load = loadavg()?;
    if r_sampling.cpu_load_one {
        point.add_field("cpu_load_one", Value::Float(load.one));
    }
    if r_sampling.cpu_load_five {
        point.add_field("cpu_load_five", Value::Float(load.five));
    }
    if r_sampling.cpu_load_fifteen {
        point.add_field("cpu_load_fifteen", Value::Float(load.fifteen));
    }

    let mem = mem_info()?;
    if r_sampling.mem_total {
        point.add_field("mem_total", Value::Integer(mem.total as i64));
    }
    if r_sampling.mem_free {
        point.add_field("mem_free", Value::Integer(mem.free as i64));
    }
    if r_sampling.mem_avail {
        point.add_field("mem_avail", Value::Integer(mem.avail as i64));
    }
    if r_sampling.mem_buffers {
        point.add_field("mem_buffers", Value::Integer(mem.buffers as i64));
    }
    if r_sampling.mem_cached {
        point.add_field("mem_cached", Value::Integer(mem.cached as i64));
    }
    if r_sampling.swap_total {
        point.add_field("swap_total", Value::Integer(mem.swap_total as i64));
    }
    if r_sampling.swap_free {
        point.add_field("swap_free", Value::Integer(mem.swap_free as i64));
    }

    let disk = disk_info()?;
    if r_sampling.disk_total {
        point.add_field("disk_total", Value::Integer(disk.total as i64));
    }
    if r_sampling.disk_free {
        point.add_field("disk_free", Value::Integer(disk.free as i64));
    }

    let proc_total = proc_total()?;
    if r_sampling.proc_total {
        point.add_field("proc_total", Value::Integer(proc_total as i64));
    }

    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?;

    point.add_timestamp(timestamp.as_millis() as i64);
    let point = point.to_owned();
    debug!("sampling_repeat: {:?}", point);

    if export_file {
        let export_file = export_path.join("system_records.csv");
        let mut export_header: bool = false;
        if !export_file.exists() {
            export_header = true;
        }

        let file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(export_file)?;

        let mut export_file = LineWriter::new(file);
        if export_header {
            const HEADER: &str = "timestamp,cpu_info_one,cpu_info_five,cpu_info_fifteen,mem_total,\
            mem_free,mem_avail,mem_buffers,mem_cached,swap_total,swap_free,disk_total,disk_free,\
            proc_total\n";
            export_file.write_all(HEADER.as_bytes())?;
        }
        let system_time = SystemTime::now();
        let datetime: DateTime<Utc> = system_time.into();

        let data = format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            datetime.format("%Y-%m-%d %H:%M:%S"),
            load.one,
            load.five,
            load.fifteen,
            mem.total,
            mem.free,
            mem.avail,
            mem.buffers,
            mem.cached,
            mem.swap_total,
            mem.swap_free,
            disk.total,
            disk.free,
            proc_total
        );
        export_file.write_all(data.as_bytes())?;
        export_file.flush()?;
    }

    Ok(point)
}

fn construct_headers(app_key: &str) -> Result<HeaderMap, Box<dyn Error>> {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert("appKey", HeaderValue::from_str(app_key)?);
    headers.insert("Accept", HeaderValue::from_static("application/json"));

    Ok(headers)
}

pub fn sampling_thingworx(
    twx_server: &ThingworxServer,
    export_path: &Path,
    export_file: bool,
) -> Result<Vec<Point>, Box<dyn Error>> {
    //let client = ReqClient::new();
    let client = ReqClient::builder()
        .gzip(true)
        .timeout(Duration::from_secs(10))
        .build()?;

    let mut points: Vec<Point> = Vec::new();
    //let mut export_points: Vec<BTreeMap<String,String>> = Vec::new();

    let test_component = match &twx_server.alias {
        Some(component) => component.clone(),
        None => format!("{}_{}", &twx_server.host, &twx_server.port),
    };

    for metric in twx_server.metrics.iter() {
        if !metric.enabled {
            continue;
        }
        let url = twx_server.get_url()?;
        let url = format!("{}/{}", url, metric.url);
        debug!("url:{}", url);
        let headers = construct_headers(&twx_server.app_key)?;
        debug!("header:{:?}", headers);
        //let mut res = client.post(&url).headers(headers).send()?;

        //reorganize output
        //measurement will be the name of metrics, for example: StreamProcessingSubsystem
        //tags will include:
        //alias
        //host_port if it's not 80 or 443
        //provider
        //each aspect will be fields, for example:
        //queueSize, totalWritesQueued, totalWritesPerformed

        // provider: name <-> value
        let mut metric_value_map: HashMap<String, BTreeMap<String, f64>> = HashMap::new();
        let system_time = SystemTime::now();
        let timestamp: DateTime<Utc> = system_time.into();

        let response_start = SystemTime::now();
        let mut response_time = 0;

        match client.post(&url).headers(headers).send() {
            Ok(mut res) => {
                if res.status().is_success() {
                    if let Ok(twx_json) = res.json::<TwxJson>() {
                        //good points after parsing (deserialization)
                        debug!("JSON:{:?}", twx_json);

                        response_time = match response_start.elapsed() {
                            Ok(elapsed) => elapsed.as_nanos(),
                            Err(_) => 0,
                        };

                        for row in twx_json.rows.iter() {
                            match &metric.options {
                                Some(option_vec) => {
                                    if !option_vec.contains(&row.name) {
                                        continue;
                                    }
                                }
                                None => {}
                            }
                            let (provider, _description) = match row.description.find(": ") {
                                Some(start) => (
                                    (&row.description[0..start]).to_string(),
                                    (&row.description[start + 2..]).to_string(),
                                ),
                                None => ("Default".to_string(), (&row.description).to_string()),
                            };

                            if !metric_value_map.contains_key(&provider) {
                                let value_map: BTreeMap<String, f64> = BTreeMap::new();
                                metric_value_map.insert(provider.clone(), value_map);
                            }

                            if let Some(value_map) = metric_value_map.get_mut(&provider) {
                                value_map.insert(row.name.clone(), row.value);
                            }
                        }
                    }
                } else {
                    //bad status (not success.)
                    debug!("status: {:?}", res);
                }
            }
            Err(e) => {
                //failed http call.
                debug!("HTTP Error:{}", e);
            }
        }

        //export data to both database and file.
        for (provider, value_map) in &metric_value_map {
            if value_map.len() == 0 {
                continue;
            }
            let mut point = Point::new(&metric.name);
            point.add_tag("Provider", Value::String(provider.clone()));
            point.add_tag("Platform", Value::String(test_component.clone()));
            point.add_timestamp(timestamp.timestamp_nanos() / 1_000_000);

            for (key, value) in value_map {
                point.add_field(key.clone(), Value::Float(*value));
            }

            point.add_field("ResponseTime", Value::Integer(response_time as i64));

            points.push(point);

            if export_file {
                let filename = format!("{}_{}.csv", &metric.name, provider);
                let export_file = export_path.join(filename);

                let file_exist = export_file.exists();

                let file = OpenOptions::new()
                    .append(true)
                    .create(true)
                    .open(export_file)?;

                let mut export_file = LineWriter::new(file);

                let mut row_headers = String::new();
                let mut row_values = String::new();
                if !file_exist {
                    row_headers.push_str("time,provider");
                    row_headers.push_str(",platform");
                }

                row_values.push_str(&format!("{},{},{}", timestamp, &provider, &test_component,));

                for (key, value) in value_map {
                    if !file_exist {
                        row_headers.push_str(",");
                        row_headers.push_str(key);
                    }

                    row_values.push_str(&format!(",{}", value));
                }
                if !file_exist {
                    row_headers.push_str("\n");
                    export_file.write_all(row_headers.as_bytes())?;
                }

                row_values.push_str("\n");
                export_file.write_all(row_values.as_bytes())?;
                export_file.flush()?;
            }
        }
    }

    debug!("points:{:?}", points.len());
    Ok(points)
}

//Ok(())

// println!("Configuration:\n{:?}", testconfig);

// println!("os: {} {}", os_type().unwrap(), os_release().unwrap());
// println!("cpu: {} cores, {} MHz", cpu_num().unwrap(), cpu_speed().unwrap());
// println!("proc total: {}", proc_total().unwrap());
// let load = loadavg().unwrap();
// println!("load: {} {} {}", load.one, load.five, load.fifteen);
// let mem = mem_info().unwrap();
// println!("mem: total {} KB, free {} KB, avail {} KB, buffers {} KB, cached {} KB",
//          mem.total, mem.free, mem.avail, mem.buffers, mem.cached);
// println!("swap: total {} KB, free {} KB", mem.swap_total, mem.swap_free);
// let disk = disk_info().unwrap();
// println!("disk: total {} KB, free {} KB", disk.total, disk.free);
// println!("hostname: {}", hostname().unwrap());
// let t = boottime().unwrap();
// println!("boottime {} sec, {} usec", t.tv_sec, t.tv_usec);

// let client = Client::new();
// let mut res = client.post("https://twx85.desheng.io/Thingworx/Subsystems/ValueStreamProcessingSubsystem/Services/GetPerformanceMetrics")
//     .headers(construct_headers())
//     .send()?;
// if res.status().is_success(){
//     println!("Success!");

//     if let Ok(twx_json) = res.json::<TwxJson>(){
//         println!("parsed successfully.");
//         //let rows = twx_json.rows;
//         //println!("{:#?}", twx_json.rows);
//         for row in twx_json.rows.iter(){
//             //println!("{:#?}", row);
//             let key=match row.description.find(": ") {
//                 Some(start) => &row.description[0..start],
//                 None => "Default",
//             };
//             println!("key:{},name:{}, \n\tvalue:{}\n\tDesc:{}", key,row.name, row.value, row.description);
//         }
//     }else{
//         println!("wrong parsing");
//     }
//     // let mut res_body = String::new();
//     // res.read_to_string(&mut res_body)?;
//     // println!("result: {}", res_body);

// }else if res.status().is_server_error(){
//     println!("Server error");
// }else{
//     println!("unknown status:{}", res.status());
// }
