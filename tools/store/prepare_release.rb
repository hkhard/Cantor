# Prepare the App Store version for review (no submission): set the version
# string to match the build, attach the build, and report what App Store
# Connect still needs. Usage (fastlane's gems, see testflight_notes.rb):
#   ... ruby -e 'gem "fastlane"; require "spaceship"; load ARGV[0]' tools/store/prepare_release.rb 1.0.0
# Key: ~/.appstoreconnect/choirbird-asc.json (private, outside the repo).
require 'spaceship'
require 'net/http'
require 'json'

TOKEN = Spaceship::ConnectAPI::Token.from(filepath: File.expand_path('~/.appstoreconnect/choirbird-asc.json'))
APP = '6838501075'
VERSION = ARGV.find { |a| a =~ /\A\d+\.\d+\.\d+\z/ } or abort 'usage: prepare_release.rb X.Y.Z'

def api(method, path, body = nil)
  uri = URI("https://api.appstoreconnect.apple.com#{path}")
  req = Net::HTTP.const_get(method.capitalize).new(uri)
  req['Authorization'] = "Bearer #{TOKEN.text}"
  req['Content-Type'] = 'application/json'
  req.body = body.to_json if body
  res = Net::HTTP.start(uri.host, uri.port, use_ssl: true) { |h| h.request(req) }
  raise "#{method} #{path}: #{res.code} #{res.body[0, 600]}" unless res.code.to_i < 300
  res.body.to_s.empty? ? {} : JSON.parse(res.body)
end

ver = api(:get, "/v1/apps/#{APP}/appStoreVersions?filter[platform]=IOS&filter[appStoreState]=PREPARE_FOR_SUBMISSION,DEVELOPER_REJECTED,REJECTED,METADATA_REJECTED")['data'].first
abort 'no editable App Store version' unless ver
if ver['attributes']['versionString'] != VERSION
  api(:patch, "/v1/appStoreVersions/#{ver['id']}", { data: { type: 'appStoreVersions', id: ver['id'], attributes: { versionString: VERSION } } })
  puts "version string: #{ver['attributes']['versionString']} -> #{VERSION}"
end

pre = api(:get, "/v1/preReleaseVersions?filter[app]=#{APP}&filter[version]=#{VERSION}")['data'].first or abort "no build for #{VERSION}"
build = api(:get, "/v1/builds?filter[preReleaseVersion]=#{pre['id']}&filter[processingState]=VALID&sort=-uploadedDate&limit=1")['data'].first or abort 'build not processed'
api(:patch, "/v1/appStoreVersions/#{ver['id']}/relationships/build", { data: { type: 'builds', id: build['id'] } })
puts "build attached: #{build['attributes']['version']}"

# Readiness, read back.
v = api(:get, "/v1/appStoreVersions/#{ver['id']}")['data']['attributes']
b = api(:get, "/v1/appStoreVersions/#{ver['id']}/build")['data']
puts "version #{v['versionString']} state #{v['appStoreState']} release #{v['releaseType']} copyright #{v['copyright'].inspect} build #{b && b['attributes']['version']}"
api(:get, "/v1/appStoreVersions/#{ver['id']}/appStoreVersionLocalizations")['data'].each do |l|
  a = l['attributes']
  sets = api(:get, "/v1/appStoreVersionLocalizations/#{l['id']}/appScreenshotSets")['data']
  shots = sets.map { |s| "#{s['attributes']['screenshotDisplayType']}=#{api(:get, "/v1/appScreenshotSets/#{s['id']}/appScreenshots")['data'].length}" }
  missing = %w[description keywords supportUrl].reject { |k| a[k].to_s.strip.size > 0 }
  puts "#{a['locale']}: text #{missing.empty? ? 'ok' : 'missing ' + missing.join(',')}; screenshots #{shots.join(' ')}"
end
r = api(:get, "/v1/appStoreVersions/#{ver['id']}/appStoreReviewDetail")['data']
puts "review contact: #{r ? [r['attributes']['contactFirstName'], r['attributes']['contactLastName'], r['attributes']['contactPhone'] ? 'phone set' : 'NO PHONE'].join(' ') : 'MISSING'}"
info = api(:get, "/v1/apps/#{APP}/appInfos")['data'].first
puts "age rating: #{info['attributes']['appStoreAgeRating'] || 'not set'}; app info state #{info['attributes']['state']}"
puts 'Not submitted. Submit for review in App Store Connect (or ask the agent).'
