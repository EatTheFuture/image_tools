use std::{
    env,
    fs::File,
    io::{self, BufRead, Write},
    path::Path,
};

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("emor.inc");
    let mut f = File::create(&dest_path).unwrap();

    let emor_table = load_emor_file("data/emor.txt");
    f.write_all(
        format!(
            "pub const EMOR_TABLE: &[[f32; {}]] = &[\n",
            emor_table[0].len()
        )
        .as_bytes(),
    )
    .unwrap();
    for list in &emor_table[..] {
        f.write_all(b"    [").unwrap();
        for value in list {
            f.write_all(format!("{:0.6}, ", value).as_bytes()).unwrap();
        }
        f.write_all(b"],\n").unwrap();
    }
    f.write_all(b"];\n\n").unwrap();

    let invemor_table = load_emor_file("data/invemor.txt");
    f.write_all(
        format!(
            "pub const INV_EMOR_TABLE: &[[f32; {}]] = &[\n",
            invemor_table[0].len()
        )
        .as_bytes(),
    )
    .unwrap();
    for list in &invemor_table[..] {
        f.write_all(b"    [").unwrap();
        for value in list {
            f.write_all(format!("{:0.6}, ", value).as_bytes()).unwrap();
        }
        f.write_all(b"],\n").unwrap();
    }
    f.write_all(b"];\n\n").unwrap();
}

fn load_emor_file(filepath: &str) -> Vec<Vec<f32>> {
    let mut values = Vec::new();
    for line in io::BufReader::new(File::open(filepath).unwrap()).lines() {
        let line = line.unwrap();

        if line.contains("=") {
            values.push(Vec::new());
        } else {
            let cur_list = values.last_mut().unwrap();
            for item in line.split(" ") {
                if let Ok(number) = item.trim().parse::<f32>() {
                    cur_list.push(number);
                }
            }
        }
    }

    for i in 0..values[0].len() {
        values[1][i] -= values[0][i];
    }

    values
}
