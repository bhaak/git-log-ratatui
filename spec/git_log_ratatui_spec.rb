# frozen_string_literal: true

require "spec_helper"

RSpec.describe GitLogRatatui do
  it "has a version number" do
    expect(GitLogRatatui::VERSION).not_to be nil
  end
end
