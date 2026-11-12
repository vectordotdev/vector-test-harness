use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use anyhow::{ensure, Context, Result};
use serde_json::{json, Map, Value};

use crate::{
    aws,
    cli::{CompareArgs, ExportArgs, ResultsArgs},
    infra,
    runtime::{identifier, Runtime},
};

type Row = Map<String, Value>;

fn sql_string(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn comparison_sql(args: &CompareArgs) -> String {
    let mut conditions = vec![];
    if let Some(test) = &args.test {
        conditions.push(format!(
            "v.name = {} AND v.configuration = {}",
            sql_string(test),
            sql_string(&args.configuration)
        ));
    }
    if let Some(subject) = &args.subject {
        conditions.push(format!("v.subject = {}", sql_string(subject)));
    }
    if args.versions.is_empty() {
        conditions.push("v.version NOT LIKE 'dev-%' AND v.version NOT LIKE 'nightly/%'".into());
    } else {
        conditions.push(
            args.versions
                .iter()
                .map(|v| format!("v.version = {}", sql_string(v)))
                .collect::<Vec<_>>()
                .join(" OR "),
        );
    }
    let where_clause = conditions
        .iter()
        .map(|s| format!("({s})"))
        .collect::<Vec<_>>()
        .join(" AND ");
    include_str!("../queries/compare.sql")
        .replace("{{where}}", &where_clause)
        .replace(
            "{{subject_group}}",
            if args.subject.is_some() {
                "v.subject, v.version"
            } else {
                "subject"
            },
        )
}

fn saved_or_query(runtime: &Runtime, input: Option<&Path>, query: &str) -> Result<Option<Value>> {
    if let Some(path) = input {
        return Ok(Some(
            serde_json::from_slice(&fs::read(runtime.path(path)?)?)
                .context("invalid Athena results JSON")?,
        ));
    }
    aws::athena(runtime, query)
}

fn athena_rows(response: &Value) -> Result<Vec<Row>> {
    let result = &response["ResultSet"];
    let columns = result["ResultSetMetadata"]["ColumnInfo"]
        .as_array()
        .context("Athena results have no column metadata")?;
    let records = result["Rows"]
        .as_array()
        .context("Athena results have no Rows array")?;
    let mut rows = vec![];
    for record in records {
        let cells = record["Data"]
            .as_array()
            .context("Athena row has no Data array")?;
        ensure!(
            cells.len() == columns.len(),
            "Athena row has {} cells for {} columns",
            cells.len(),
            columns.len()
        );
        if cells
            .iter()
            .zip(columns)
            .all(|(cell, col)| cell["VarCharValue"] == col["Name"])
        {
            continue;
        }
        let mut row = Map::new();
        for (cell, column) in cells.iter().zip(columns) {
            let name = column["Name"].as_str().context("column has no name")?;
            let value = match cell.get("VarCharValue").and_then(Value::as_str) {
                None => Value::Null,
                Some(value) => match column["Type"].as_str().unwrap_or("varchar") {
                    "bigint" | "integer" | "smallint" | "tinyint" => json!(value
                        .parse::<i64>()
                        .with_context(|| format!("invalid integer for {name}"))?),
                    "double" | "float" | "real" | "decimal" => {
                        let number: f64 = value
                            .parse()
                            .with_context(|| format!("invalid number for {name}"))?;
                        ensure!(number.is_finite(), "non-finite number for {name}");
                        json!(number)
                    }
                    _ => Value::String(value.into()),
                },
            };
            row.insert(name.into(), value);
        }
        rows.push(row);
    }
    Ok(rows)
}

fn text<'a>(row: &'a Row, key: &str) -> Result<&'a str> {
    row.get(key)
        .and_then(Value::as_str)
        .with_context(|| format!("missing {key} in measurement"))
}

fn table(headers: &[String], rows: &[Vec<String>]) {
    let widths: Vec<_> = (0..headers.len())
        .map(|i| {
            rows.iter()
                .filter_map(|r| r.get(i))
                .chain(std::iter::once(&headers[i]))
                .map(|s| s.chars().count())
                .max()
                .unwrap_or(0)
        })
        .collect();
    for row in std::iter::once(headers).chain(rows.iter().map(Vec::as_slice)) {
        println!(
            "{}",
            row.iter()
                .zip(&widths)
                .map(|(s, w)| format!("{s}{}", " ".repeat(w - s.chars().count())))
                .collect::<Vec<_>>()
                .join("  ")
        );
    }
}

fn filesize(value: f64) -> String {
    let units = ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"];
    let mut value = value;
    let mut unit = 0;
    while value.abs() >= 1024.0 && unit + 1 < units.len() {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", units[unit])
}

pub fn compare(runtime: &Runtime, args: CompareArgs) -> Result<()> {
    if args.repair && args.input.is_none() {
        aws::athena(runtime, "MSCK REPAIR TABLE vector_tests")?;
    }
    let Some(response) = saved_or_query(runtime, args.input.as_deref(), &comparison_sql(&args))?
    else {
        return Ok(());
    };
    let rows = athena_rows(&response)?;
    ensure!(!rows.is_empty(), "no matching measurements");
    if args.json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
        return Ok(());
    }
    let metadata: BTreeMap<String, Value> =
        serde_json::from_slice(&fs::read(runtime.root.join("config/metrics.json"))?)?;
    let labels = rows
        .iter()
        .map(|row| {
            Ok(format!(
                "{} {}",
                text(row, "subject")?,
                text(row, "version")?
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let mut headers = vec!["Metric".into()];
    headers.extend(labels);
    let mut display = vec![std::iter::once("Test count".into())
        .chain(
            rows.iter()
                .map(|r| r.get("count").map_or("n/a".into(), Value::to_string)),
        )
        .collect()];
    let mut metrics: Vec<_> = metadata
        .iter()
        .filter(|(key, _)| rows.iter().any(|r| r.get(*key).is_some()))
        .collect();
    metrics.sort_by_key(|(_, meta)| meta["name"].as_str().unwrap_or_default());
    for (key, meta) in metrics {
        let values: Vec<_> = rows
            .iter()
            .map(|r| r.get(key).and_then(Value::as_f64))
            .collect();
        let reference = values.first().copied().flatten();
        let mut line = vec![meta["name"].as_str().unwrap_or(key).to_owned()];
        for value in values {
            let formatted = match value {
                None => "n/a".into(),
                Some(value) => {
                    let base = if meta["unit"] == "bytes" {
                        filesize(value)
                    } else {
                        format!("{value:.2}")
                    };
                    match reference {
                        Some(reference)
                            if args.percentages && !args.no_percentages && reference != 0.0 =>
                        {
                            format!("{base} ({:+.1}%)", (value - reference) / reference * 100.0)
                        }
                        _ => base,
                    }
                }
            };
            line.push(formatted);
        }
        display.push(line);
    }
    table(&headers, &display);
    println!("W = winner");
    Ok(())
}

fn metric_column(test: &str) -> usize {
    match test {
        "file_to_tcp_performance" | "tcp_to_http_performance" => 18,
        _ => 17,
    }
}

fn mibps(content: &str, column: usize) -> Result<Option<f64>> {
    // Preserve bin/results' historical positive-sample selection and trimming.
    let body = content.lines().skip(8).collect::<Vec<_>>().join("\n");
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(body.as_bytes());
    let mut samples = vec![];
    for record in reader.records() {
        let record = record.context("invalid dstat CSV")?;
        if record
            .get(0)
            .and_then(|s| s.trim().parse::<f64>().ok())
            .is_none()
        {
            continue;
        }
        if let Some(value) = record
            .get(column)
            .and_then(|s| s.trim().parse::<f64>().ok())
        {
            if value.is_finite() && value > 0.0 {
                samples.push(value);
            }
        }
    }
    if samples.is_empty() {
        return Ok(None);
    }
    let steady = if samples.len() > 20 {
        &samples[10..samples.len() - 5]
    } else {
        &samples
    };
    Ok(Some(
        steady.iter().sum::<f64>() / steady.len() as f64 / 1_048_576.0,
    ))
}

fn partitions(key: &str) -> BTreeMap<&str, &str> {
    key.split('/')
        .filter_map(|segment| segment.split_once('='))
        .collect()
}

pub fn results(runtime: &Runtime, args: ResultsArgs) -> Result<()> {
    identifier(&args.test, "test name")?;
    ensure!(
        args.user_id.is_none(),
        "historical result keys have no user ID partition; --user-id cannot reliably filter them"
    );
    let column = metric_column(&args.test);
    let mut measurements = vec![];
    if !args.input.is_empty() {
        for input in &args.input {
            let path = runtime.path(input)?;
            let value = mibps(&fs::read_to_string(&path)?, column)?;
            measurements.push(json!({"subject":path.file_name().unwrap_or_default().to_string_lossy(), "version":"local", "mib_per_second":value}));
        }
    } else {
        let bucket = args
            .bucket
            .or_else(|| runtime.get("VECTOR_TEST_RESULTS_S3_BUCKET_NAME"))
            .context("supply --bucket or VECTOR_TEST_RESULTS_S3_BUCKET_NAME")?;
        let mut prefix = format!("name={}/", args.test);
        if let Some(configuration) = &args.configuration {
            identifier(configuration, "configuration")?;
            prefix.push_str(&format!("configuration={configuration}/"));
        }
        let spec = aws::command(
            runtime,
            [
                "s3api",
                "list-objects-v2",
                "--bucket",
                &bucket,
                "--prefix",
                &prefix,
            ],
        );
        if runtime.dry_run {
            return runtime.plan(&spec);
        }
        let listing: Value = serde_json::from_str(&runtime.capture(spec)?)?;
        let mut latest = BTreeMap::<(String, String, String), (u64, String)>::new();
        for object in listing["Contents"].as_array().into_iter().flatten() {
            let key = object["Key"].as_str().context("S3 object has no key")?;
            if !key.ends_with("/dstat.csv") {
                continue;
            }
            let p = partitions(key);
            let group = (
                p.get("configuration").unwrap_or(&"?").to_string(),
                p.get("subject").unwrap_or(&"?").to_string(),
                p.get("version").unwrap_or(&"?").to_string(),
            );
            let timestamp = p
                .get("timestamp")
                .context("result key has no timestamp")?
                .parse::<u64>()
                .context("invalid result timestamp")?;
            let candidate = (timestamp, key.to_owned());
            if latest.get(&group).is_none_or(|old| old < &candidate) {
                latest.insert(group, candidate);
            }
        }
        ensure!(
            !latest.is_empty(),
            "no dstat CSVs found under s3://{bucket}/{prefix}"
        );
        for ((configuration, subject, version), (_, key)) in latest {
            let content = runtime.capture(aws::command(
                runtime,
                ["s3", "cp", &format!("s3://{bucket}/{key}"), "-"],
            ))?;
            measurements.push(json!({"configuration":configuration,"subject":subject,"version":version,"mib_per_second":mibps(&content, column)?}));
        }
    }
    measurements.sort_by(|a, b| {
        b["mib_per_second"]
            .as_f64()
            .unwrap_or(-1.0)
            .total_cmp(&a["mib_per_second"].as_f64().unwrap_or(-1.0))
    });
    if args.json {
        println!("{}", serde_json::to_string_pretty(&measurements)?);
    } else {
        let rows = measurements
            .iter()
            .map(|r| {
                vec![
                    r["configuration"].as_str().unwrap_or("local").into(),
                    r["subject"].as_str().unwrap_or("?").into(),
                    r["version"].as_str().unwrap_or("?").into(),
                    r["mib_per_second"]
                        .as_f64()
                        .map_or("n/a".into(), |n| format!("{n:.1}")),
                ]
            })
            .collect::<Vec<_>>();
        table(
            &[
                "Configuration".into(),
                "Subject".into(),
                "Version".into(),
                "MiB/s".into(),
            ],
            &rows,
        );
    }
    Ok(())
}

fn unique(rows: &[Row], key: &str) -> Vec<String> {
    rows.iter()
        .filter_map(|r| r.get(key).and_then(Value::as_str))
        .map(str::to_owned)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn metric_keys(rows: &[Row], metadata: &BTreeMap<String, Value>) -> Vec<String> {
    metadata
        .keys()
        .filter(|key| rows.iter().any(|row| row.contains_key(*key)))
        .cloned()
        .collect()
}

fn version(value: &str) -> Value {
    let normalized = value.strip_prefix('v').unwrap_or(value);
    let core = normalized.split(['-', '+']).next().unwrap_or(normalized);
    let components: Vec<_> = core
        .split('.')
        .map(|part| part.parse::<u64>().ok())
        .collect();
    json!({"__type":"version", "major":components.first().copied().flatten(), "minor":components.get(1).copied().flatten(), "patch":components.get(2).copied().flatten(), "name":normalized, "prerelease":normalized.contains('-') || components.first().is_none_or(|p| p.is_none()), "slug":value})
}

fn web_export(runtime: &Runtime, mut rows: Vec<Row>) -> Result<Value> {
    let metadata: BTreeMap<String, Value> =
        serde_json::from_slice(&fs::read(runtime.root.join("config/metrics.json"))?)?;
    for row in &mut rows {
        let metric = if text(row, "name")?.contains("file") {
            "disk_read_avg"
        } else {
            "net_recv_avg"
        };
        row.insert(
            "throughput_avg".into(),
            row.get(metric).cloned().unwrap_or(Value::Null),
        );
    }
    let mut measurements = vec![];
    for row in &rows {
        for key in metric_keys(std::slice::from_ref(row), &metadata) {
            let Some(value) = row[&key].as_f64() else {
                continue;
            };
            let mut measurement = metadata[&key]
                .as_object()
                .context("invalid metric metadata")?
                .clone();
            measurement.extend(json!({"__type":"measurement","human_value":if measurement["unit"] == "bytes" { filesize(value) } else { format!("{value:.2}") },"metric":key,"subject":text(row,"subject")?,"test":text(row,"name")?,"value":value,"version":text(row,"version")?}).as_object().unwrap().clone());
            measurements.push(Value::Object(measurement));
        }
    }
    let mut metrics = vec![];
    for key in metric_keys(&rows, &metadata) {
        let relevant: Vec<_> = rows
            .iter()
            .filter(|r| r.get(&key).is_some_and(|v| !v.is_null()))
            .cloned()
            .collect();
        let mut metric = metadata[&key]
            .as_object()
            .context("invalid metric metadata")?
            .clone();
        metric.extend(json!({"__type":"metric","slug":key,"subjects":unique(&relevant,"subject"),"tests":unique(&relevant,"name")}).as_object().unwrap().clone());
        metrics.push(Value::Object(metric));
    }
    metrics.sort_by_key(|m| m["name"].as_str().unwrap_or_default().to_owned());
    let catalog = infra::subjects(runtime)?;
    let mut subjects = vec![];
    for slug in unique(&rows, "subject") {
        let selected: Vec<_> = rows
            .iter()
            .filter(|r| r.get("subject").and_then(Value::as_str) == Some(&slug))
            .cloned()
            .collect();
        let name = catalog
            .iter()
            .find(|s| s.slug == slug)
            .map_or(slug.as_str(), |s| s.name.as_str());
        subjects.push(json!({"__type":"subject","name":name,"metrics":metric_keys(&selected,&metadata),"slug":slug,"tests":unique(&selected,"name"),"versions":unique(&selected,"version").iter().map(|s|version(s)).collect::<Vec<_>>()}));
    }
    subjects.sort_by_key(|s| {
        if s["slug"] == "vector" {
            String::new()
        } else {
            s["name"].as_str().unwrap_or_default().to_owned()
        }
    });
    let mut tests = vec![];
    for slug in unique(&rows, "name") {
        identifier(&slug, "test name")?;
        let selected: Vec<_> = rows
            .iter()
            .filter(|r| r.get("name").and_then(Value::as_str) == Some(&slug))
            .cloned()
            .collect();
        let readme = fs::read_to_string(runtime.root.join("cases").join(&slug).join("README.md"))
            .unwrap_or_default();
        let name = match slug.as_str() {
            "file_to_tcp_performance" => "File To TCP",
            "regex_parsing_performance" => "Regex Parsing",
            "tcp_to_blackhole_performance" => "TCP To Blackhole",
            "tcp_to_http_performance" => "TCP To HTTP",
            "tcp_to_tcp_performance" => "TCP To TCP",
            _ => &slug,
        };
        tests.push(json!({"__type":"test","name":name,"description":readme.split("\n\n").nth(1).unwrap_or_default(),"metrics":metric_keys(&selected,&metadata),"slug":slug,"subjects":unique(&selected,"subject")}));
    }
    tests.sort_by_key(|t| t["name"].as_str().unwrap_or_default().to_owned());
    Ok(
        json!({"performance_tests":{"tests":tests,"subjects":subjects,"measurements":measurements,"metrics":metrics}}),
    )
}

pub fn export(runtime: &Runtime, args: ExportArgs) -> Result<()> {
    ensure!(
        args.no_upload,
        "website publication is disabled: remote performance comparisons have been retired; use --no-upload to inspect archived data"
    );
    let Some(response) = saved_or_query(
        runtime,
        args.input.as_deref(),
        include_str!("../queries/export.sql"),
    )?
    else {
        return Ok(());
    };
    let rows = athena_rows(&response)?;
    ensure!(!rows.is_empty(), "no measurements to export");
    let document = web_export(runtime, rows)?;
    let output = runtime.path(args.output)?;
    runtime.write(&output, serde_json::to_string_pretty(&document)?.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn historical_sample_selection_and_trimming() {
        let mut content = "header\n".repeat(8);
        for i in 0..30 {
            let mut row = vec!["0".to_owned(); 19];
            row[0] = i.to_string();
            row[17] = ((i + 1) * 1_048_576).to_string();
            content.push_str(&row.join(","));
            content.push('\n');
        }
        assert_eq!(mibps(&content, 17).unwrap(), Some(18.0));
    }

    #[test]
    fn missing_athena_values_stay_null() {
        let response = json!({"ResultSet":{"ResultSetMetadata":{"ColumnInfo":[{"Name":"duration_avg","Type":"double"}]},"Rows":[{"Data":[{"VarCharValue":"duration_avg"}]},{"Data":[{}]}]}});
        assert!(athena_rows(&response).unwrap()[0]["duration_avg"].is_null());
    }

    #[test]
    fn query_values_are_sql_literals() {
        assert_eq!(sql_string("v1' OR true --"), "'v1'' OR true --'");
    }
}
