require_relative "util"

def get_column!(result_set_metadata, name)
  column_info = result_set_metadata["ColumnInfo"]

  column_info.each do |column|
    if column["Name"] == name
      return column
    end
  end

  raise "Column #{name} could not be found!"
end

def coerce!(value, column)
  type = column["Type"]
  case type
  when "bigint"
    Integer(value)
  when "double"
    Float(value)
  when "varchar"
    value
  else
    raise "Unrecognized column type #{type}!"
  end
end

def transform_to_hashes!(rows, result_set_metadata)
  hashes = []

  keys = rows[0]["Data"].collect { |header| header["VarCharValue"] }

  rows[1..-1].each do |row|
    hash = {}
    keys.each_with_index do |key, index|
      value = row["Data"][index]["VarCharValue"]
      column = get_column!(result_set_metadata, key)
      value = coerce!(value, column)
      hash[key] = value
    end

    hash["short_subject"] = build_short_subject_name(hash["subject"])

    hashes << hash
  end

  hashes
end
