# frozen_string_literal: true

require_relative "git_log_ratatui/version"
require_relative "git_log_ratatui/git"
require_relative "git_log_ratatui/app"

module GitLogRatatui
  class Error < StandardError; end
end
