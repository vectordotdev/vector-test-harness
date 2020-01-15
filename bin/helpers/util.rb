AGGREGATE_FUNS = ["avg", "max", "min", "sum"]

METRIC_META = {
  'cpu_sys_avg' => {
    category: "CPU",
    name: "CPU Sys (avg)",
    aggregate: "avg",
    unit: "load",
    sort: :asc
  },
  'cpu_sys_max' => {
    category: "CPU",
    name: "CPU Sys (max)",
    aggregate: "max",
    unit: "load",
    sort: :asc
  },
  'cpu_usr_avg' => {
    category: "CPU",
    name: "CPU Usr (avg)",
    aggregate: "avg",
    unit: "load",
    sort: :asc
  },
  'cpu_usr_max' => {
    category: "CPU",
    name: "CPU Usr (max)",
    aggregate: "max",
    unit: "load",
    sort: :asc
  },
  'duration_avg' => {
    category: "Duration",
    name: "Duration (avg)",
    aggregate: "avg",
    unit: "s",
    sort: :asc
  },
  'duration_max' => {
    category: "Duration",
    name: "Duration (max)",
    aggregate: "max",
    unit: "s",
    sort: :asc
  },
  'disk_read_avg' => {
    category: "IO",
    name: "Disk Read (avg)",
    aggregate: "avg",
    unit: "bytes",
    sort: :asc
  },
  'disk_read_sum' => {
    category: "IO",
    name: "Disk Read (total)",
    aggregate: "sum",
    unit: "bytes",
    sort: :desc
  },
  'disk_writ_sum' => {
    category: "IO",
    name: "Disk Write (total)",
    aggregate: "sum",
    unit: "bytes",
    sort: :desc
  },
  'io_read_sum' => {
    category: "IO",
    name: "IO Read (total)",
    aggregate: "sum",
    unit: "bytes",
    sort: :desc
  },
  'io_writ_sum' => {
    category: "IO",
    name: "IO Write (total)",
    aggregate: "sum",
    unit: "bytes",
    sort: :desc
  },
  'load_avg_1m' => {
    category: "CPU",
    name: "CPU Load 1m (avg)",
    aggregate: "avg",
    unit: "load",
    sort: :asc
  },
  'mem_used_avg' => {
    category: "Mem",
    name: "Mem Used (avg)",
    aggregate: "avg",
    unit: "bytes",
    sort: :asc
  },
  'mem_used_max' => {
    category: "Mem",
    name: "Mem Used (max)",
    aggregate: "max",
    unit: "bytes",
    sort: :asc
  },
  'net_recv_avg' => {
    category: "IO",
    name: "Net Recv (avg)",
    aggregate: "avg",
    unit: "bytes",
    sort: :asc
  },
  'net_recv_sum' => {
    category: "IO",
    name: "Net Recv (total)",
    aggregate: "sum",
    unit: "bytes",
    sort: :desc
  },
  'net_send_sum' => {
    category: "IO",
    name: "Net Send (total)",
    aggregate: "sum",
    unit: "bytes",
    sort: :desc
  },
  'sock_total_sum' => {
    category: "Socket",
    name: "Socket Total (total)",
    aggregate: "sum",
    unit: "conns",
    sort: :asc
  },
  'tcp_act_sum' => {
    category: "Socket",
    name: "TCP ACT (total)",
    aggregate: "sum",
    unit: "conns",
    sort: :asc
  },
  'tcp_syn_sum' => {
    category: "Socket",
    name: "TCP Syn (total)",
    aggregate: "sum",
    unit: "syns",
    sort: :asc
  },
  'tcp_clo_sum' => {
    category: "Socket",
    name: "TCP Close (total)",
    aggregate: "sum",
    unit: "closes",
    sort: :asc
  },
  'throughput_avg' => {
    category: "IO",
    name: "Throughput (avg)",
    aggregate: "avg",
    unit: "bytes",
    sort: :desc
  }
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
    name: 'File To TCP'
  },
  'regex_parsing_performance' => {
    name: 'Regex Parsing'
  },
  'tcp_to_blackhole_performance' => {
    name: 'TCP To Blackhole'
  },
  'tcp_to_http_performance' => {
    name: 'TCP To HTTP'
  },
  'tcp_to_tcp_performance' => {
    name: 'TCP To TCP'
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
  content = File.read("#{File.dirname(__FILE__)}/../../cases/#{slug}/README.md")
  sections = content.split(/\n\n/)

  meta = TEST_META.fetch(slug)

  meta.merge(
    description: sections.fetch(1)
  )
end

def filesize(size)
  units = ['b', 'kib', 'MiB', 'gib', 'tib', 'pib', 'eib']

  return '0.0 B' if size == 0
  exp = (Math.log(size) / Math.log(1024)).to_i
  exp = 6 if exp > 6 

  ('%.1f %s' % [size.to_f / 1024 ** exp, units[exp]]).gsub('.0', '')
end

def is_metric?(metric_slug)
  AGGREGATE_FUNS.any? do |agg|
    metric_slug.include?(agg)
  end
end