WITH
latest_versions AS (
  SELECT
    subject,
    MAX(version) AS version
  FROM vector_tests v
  WHERE {{where}}
  GROUP BY {{subject_group}}
)

SELECT
  name,
  configuration,
  subject,
  version,
  COUNT(*) AS count,
  AVG(duration) AS duration_avg,
  MAX(duration) AS duration_max,
  AVG(cpu_sys_avg) AS cpu_sys_avg,
  AVG(cpu_usr_avg) AS cpu_usr_avg,
  AVG(cpu_sys_max) AS cpu_sys_max,
  AVG(cpu_usr_max) AS cpu_usr_max,
  AVG(load_avg_1m) AS load_avg_1m,
  AVG(mem_used_avg) AS mem_used_avg,
  AVG(mem_used_max) AS mem_used_max,
  AVG(disk_read_avg) AS disk_read_avg,
  AVG(disk_read_sum) AS disk_read_sum,
  AVG(disk_writ_sum) AS disk_writ_sum,
  AVG(io_read_sum) AS io_read_sum,
  AVG(io_writ_sum) AS io_writ_sum,
  AVG(net_recv_avg) AS net_recv_avg,
  AVG(net_recv_sum) AS net_recv_sum,
  AVG(net_send_sum) AS net_send_sum,
  AVG(sock_total_sum) AS sock_total_sum,
  AVG(tcp_act_sum) AS tcp_act_sum,
  AVG(tcp_syn_sum) AS tcp_syn_sum,
  AVG(tcp_clo_sum) AS tcp_clo_sum
FROM (
  SELECT
    v.name,
    v.configuration,
    v.subject,
    v.version,
    v.timestamp,
    MAX(epoch) - MIN(epoch) AS duration,
    AVG(cpu_sys) AS cpu_sys_avg,
    AVG(cpu_usr) AS cpu_usr_avg,
    MAX(cpu_sys) AS cpu_sys_max,
    MAX(cpu_usr) AS cpu_usr_max,
    AVG(load_avg_1m) AS load_avg_1m,
    AVG(mem_used) AS mem_used_avg,
    MAX(mem_used) AS mem_used_max,
    (SUM(disk_read) / (MAX(epoch) - MIN(epoch))) AS disk_read_avg,
    SUM(disk_read) AS disk_read_sum,
    SUM(disk_writ) AS disk_writ_sum,
    SUM(io_read) AS io_read_sum,
    SUM(io_writ) AS io_writ_sum,
    (SUM(net_recv) / (MAX(epoch) - MIN(epoch))) AS net_recv_avg,
    SUM(net_recv) AS net_recv_sum,
    SUM(net_send) AS net_send_sum,
    SUM(sock_total) AS sock_total_sum,
    SUM(tcp_act) AS tcp_act_sum,
    SUM(tcp_syn) AS tcp_syn_sum,
    SUM(tcp_clo) AS tcp_clo_sum
  FROM vector_tests v
  INNER JOIN latest_versions ON
    latest_versions.subject = v.subject
    AND latest_versions.version = v.version
  WHERE {{where}}
  GROUP BY v.name, v.configuration, v.subject, v.version, v.timestamp
)
GROUP BY name, configuration, subject, version
ORDER BY subject, version ASC
