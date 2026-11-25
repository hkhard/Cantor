# After an upload: wait until App Store Connect has processed the build of a
# version, set its "What to Test" in Swedish and English from
# docs/store/testflight.md (the section "## What to Test (build X)"), add it
# to every internal beta group that does not get all builds automatically,
# and print the beta groups. External groups are only listed (adding a build
# there starts Beta App Review: done by hand, see testflight.md).
#   D=$(dirname $(readlink -f $(which fastlane)))/../libexec
#   LC_ALL=en_US.UTF-8 GEM_HOME=$D GEM_PATH=$D /opt/homebrew/opt/ruby/bin/ruby \
#     -e 'gem "fastlane"; require "spaceship"; load ARGV[0]' tools/store/testflight_notes.rb 0.3.3
# Key: ~/.appstoreconnect/choirbird-asc.json (private, outside the repo).
require 'spaceship'
require 'net/http'
require 'json'

TOKEN = Spaceship::ConnectAPI::Token.from(filepath: File.expand_path('~/.appstoreconnect/choirbird-asc.json'))
APP = '6838501075'
VERSION = ARGV.find { |a| a =~ /\A\d+\.\d+\.\d+\z/ } or abort 'usage: testflight_notes.rb X.Y.Z'

def api(method, path, body = nil)
  uri = URI("https://api.appstoreconnect.apple.com#{path}")
  req = Net::HTTP.const_get(method.capitalize).new(uri)
  req['Authorization'] = "Bearer #{TOKEN.text}"
  req['Content-Type'] = 'application/json'
  req.body = body.to_json if body
  res = Net::HTTP.start(uri.host, uri.port, use_ssl: true) { |h| h.request(req) }
  raise "#{method} #{path}: #{res.code} #{res.body[0, 800]}" unless res.code.to_i < 300
  res.body.to_s.empty? ? {} : JSON.parse(res.body)
end

# The two texts: "Svenska:" and "English:" blocks under the version's heading.
doc = File.read(File.expand_path('../../docs/store/testflight.md', __dir__))
sec = doc[/^## What to Test \(build #{Regexp.escape(VERSION)}[^\n]*\n(.*?)(?=^## )/m, 1] or abort "no What to Test section for #{VERSION}"
sv = sec[/^Svenska:\n(.*?)(?=^English:)/m, 1].to_s.strip
en = sec[/^English:\n(.*)/m, 1].to_s.strip
abort 'empty text' if sv.empty? || en.empty?
TEXT = { 'sv' => sv, 'en-US' => en }

# Wait for the build (processing takes a few minutes, up to ~30).
build = nil
60.times do
  pre = api(:get, "/v1/preReleaseVersions?filter[app]=#{APP}&filter[version]=#{VERSION}")['data'].first
  if pre
    build = api(:get, "/v1/builds?filter[preReleaseVersion]=#{pre['id']}&sort=-uploadedDate&limit=1")['data'].first
    state = build && build['attributes']['processingState']
    puts "build #{build && build['attributes']['version']}: #{state}"
    break if state == 'VALID'
    abort "processing #{state}" if %w[FAILED INVALID].include?(state)
  else
    puts 'not visible yet'
  end
  sleep 30
end
abort 'build not processed in time' unless build && build['attributes']['processingState'] == 'VALID'

locs = api(:get, "/v1/builds/#{build['id']}/betaBuildLocalizations")['data']
TEXT.each do |locale, text|
  loc = locs.find { |l| l['attributes']['locale'] == locale }
  if loc
    api(:patch, "/v1/betaBuildLocalizations/#{loc['id']}",
        { data: { type: 'betaBuildLocalizations', id: loc['id'], attributes: { whatsNew: text } } })
  else
    api(:post, '/v1/betaBuildLocalizations',
        { data: { type: 'betaBuildLocalizations', attributes: { locale: locale, whatsNew: text },
                  relationships: { build: { data: { type: 'builds', id: build['id'] } } } } })
  end
  back = api(:get, "/v1/builds/#{build['id']}/betaBuildLocalizations")['data'].find { |l| l['attributes']['locale'] == locale }
  ok = back && back['attributes']['whatsNew'] == text
  puts "What to Test #{locale}: #{ok ? 'set' : 'MISMATCH'} (#{text.length} chars)"
end

api(:get, "/v1/apps/#{APP}/betaGroups")['data'].each do |g|
  a = g['attributes']
  kind = a['isInternalGroup'] ? 'internal' : 'external'
  if a['isInternalGroup'] && !a['hasAccessToAllBuilds']
    api(:post, "/v1/betaGroups/#{g['id']}/relationships/builds", { data: [{ type: 'builds', id: build['id'] }] })
    puts "group #{a['name']} (#{kind}): build added"
  else
    puts "group #{a['name']} (#{kind}#{a['hasAccessToAllBuilds'] ? ', gets every build' : ''})"
  end
end
