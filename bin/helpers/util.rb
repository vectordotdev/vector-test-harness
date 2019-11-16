AGGREGATE_FUNS = ["avg", "max", "min", "sum"]

METRIC_META = {
  'cpu_sys_avg' => {
    name: "CPU Sys",
    aggregate: "avg",
    unit: "load",
    sort: :asc
  },
  'cpu_sys_max' => {
    name: "CPU Sys",
    aggregate: "max",
    unit: "load",
    sort: :asc
  },
  'cpu_usr_avg' => {
    name: "CPU Usr",
    aggregate: "avg",
    unit: "load",
    sort: :asc
  },
  'cpu_usr_max' => {
    name: "CPU Usr",
    aggregate: "max",
    unit: "load",
    sort: :asc
  },
  'duration_avg' => {
    name: "Duration",
    aggregate: "avg",
    unit: "s",
    sort: :asc
  },
  'duration_max' => {
    name: "Duration",
    aggregate: "max",
    unit: "s",
    sort: :asc
  },
  'disk_read_avg' => {
    name: "Disk Read",
    aggregate: "avg",
    unit: "bytes",
    sort: :asc
  },
  'disk_read_sum' => {
    name: "Disk Read",
    aggregate: "sum",
    unit: "bytes",
    sort: :desc
  },
  'disk_writ_sum' => {
    name: "Disk Write",
    aggregate: "sum",
    unit: "bytes",
    sort: :desc
  },
  'io_read_sum' => {
    name: "IO Read",
    aggregate: "sum",
    unit: "bytes",
    sort: :desc
  },
  'io_writ_sum' => {
    name: "IO Write",
    aggregate: "sum",
    unit: "bytes",
    sort: :desc
  },
  'load_avg_1m' => {
    name: "CPU Load 1m",
    aggregate: "avg",
    unit: "load",
    sort: :asc
  },
  'mem_used_avg' => {
    name: "Mem Used",
    aggregate: "avg",
    unit: "bytes",
    sort: :asc
  },
  'mem_used_max' => {
    name: "Mem Used",
    aggregate: "max",
    unit: "bytes",
    sort: :asc
  },
  'net_recv_avg' => {
    name: "Net Recv",
    aggregate: "avg",
    unit: "bytes",
    sort: :asc
  },
  'net_recv_sum' => {
    name: "Net Recv",
    aggregate: "sum",
    unit: "bytes",
    sort: :desc
  },
  'net_send_sum' => {
    name: "Net Send",
    aggregate: "sum",
    unit: "bytes",
    sort: :desc
  },
  'sock_total_sum' => {
    name: "Socket Total",
    aggregate: "sum",
    unit: "conns",
    sort: :asc
  },
  'tcp_act_sum' => {
    name: "TCP ACT",
    aggregate: "sum",
    unit: "conns",
    sort: :asc
  },
  'tcp_syn_sum' => {
    name: "TCP Syn",
    aggregate: "sum",
    unit: "syns",
    sort: :asc
  },
  'tcp_clo_sum' => {
    name: "TCP Close",
    aggregate: "sum",
    unit: "closes",
    sort: :asc
  },
}

SUBJECT_META = {
  'filebeat' => {
    name: "Filebeat"
  },
  'fluentbit' => {
    name: "FluentBit"
  },
  'fluentd' => {
    name: "FluentD"
  },
  'logstash' => {
    name: "Logstash"
  },
  'splunk_heavy_forwarder' => {
    name: "Splunk HF"
  },
  'splunk_universal_forwarder' => {
    name: "Splunk UF"
  },
  'vector' => {
    name: "Vector"
  }
}

TEST_META = {
  'file_to_tcp_performance' => {
    name: 'File To TCP',
    description: 'A file tailing test, tailing 10 files and forwarding data over TCP',
    io_metric_slug: 'disk_read_avg'
  },
  'regex_parsing_performance' => {
    name: 'Regex Parsing',
    description: 'A Regex test, parsing Apache common log formats',
    io_metric_slug: 'net_recv_avg'
  },
  'tcp_to_blackhole_performance' => {
    name: 'TCP To Blackhole',
    description: 'A raw internal performance test, accepting data over TCP and discarding it',
    io_metric_slug: 'net_recv_avg'
  },
  'tcp_to_http_performance' => {
    name: 'TCP To HTTP',
    description: 'An HTTP performance test, accepting data over TCP and forwarding it over HTTP',
    io_metric_slug: 'net_recv_avg'
  },
  'tcp_to_tcp_performance' => {
    name: 'TCP To TCP',
    description: 'A TCP performance test, accepting data over TCP and forwarding it over TCP',
    io_metric_slug: 'net_recv_avg'
  }
}

def build_short_subject_name(name)
  case name
  when 'splunk_heavy_forwarder'
    'splunk_hf'
  when 'splunk_universal_forwarder'
    'splunk_uf'
  else
    name
  end
end

def fetch_metric_meta!(slug)
  METRIC_META.fetch(slug)
end

def fetch_subject_meta!(slug)
  SUBJECT_META.fetch(slug)
end

def fetch_test_meta!(slug)
  TEST_META.fetch(slug)
end

def filesize(size)
  units = ['b', 'kib', 'MiB', 'gib', 'tib', 'pib', 'eib']

  return '0.0 B' if size == 0
  exp = (Math.log(size) / Math.log(1024)).to_i
  exp = 6 if exp > 6 

  ('%.1f%s' % [size.to_f / 1024 ** exp, units[exp]]).gsub('.0', '')
end

def is_metric?(metric_slug)
  AGGREGATE_FUNS.any? do |agg|
    metric_slug.include?(agg)
  end
end