# Upload ChoirBird's App Store creative asset (the 5244 x 2950 universal PNG
# from tools/store/banner.js) for each language and place it as both the
# product page header and the search results asset on the version being
# prepared. App Store Connect API, App Asset Library (4.5.1+): reserve an
# appAssetLibraryImages (CREATIVE_ASSETS), PUT the parts, commit, then one
# appAssetLibraryPlacements per placement type and localization (an old
# placement of the same type is deleted first). No submission for review.
#   bash tools/store/make_banners.sh --upload
# Key: ~/.appstoreconnect/choirbird-asc.json (private, outside the repo).
require 'spaceship'
require 'net/http'
require 'json'

TOKEN = Spaceship::ConnectAPI::Token.from(filepath: File.expand_path('~/.appstoreconnect/choirbird-asc.json'))
APP = '6838501075' # ChoirBird's App Store Connect app id (also its asset library id)
# Loaded by make_banners.sh as `ruby -e '...load ARGV[0]' THIS_FILE DIR`, so the
# folder is the last argument (when it is a directory).
DIR = (ARGV.last && File.directory?(ARGV.last)) ? ARGV.last : File.expand_path('../../store-out/banners', __dir__)
FILES = { 'en-US' => File.join(DIR, 'universal-en.png'), 'sv' => File.join(DIR, 'universal-sv.png') }
TYPES = %w[PRODUCT_PAGE_HEADER_ASSET APP_STORE_SEARCH_RESULTS_ASSET].freeze

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

ver = api(:get, "/v1/apps/#{APP}/appStoreVersions?filter[appStoreState]=PREPARE_FOR_SUBMISSION&filter[platform]=IOS")['data'].first
raise 'no version in Prepare for Submission' unless ver
locs = api(:get, "/v1/appStoreVersions/#{ver['id']}/appStoreVersionLocalizations")['data']

FILES.each do |locale, path|
  loc = locs.find { |l| l['attributes']['locale'] == locale } or raise "no #{locale} localization"
  data = File.binread(path)
  img = api(:post, '/v1/appAssetLibraryImages', { data: { type: 'appAssetLibraryImages',
    attributes: { category: 'CREATIVE_ASSETS', fileName: "choirbird-#{File.basename(path)}", fileSize: data.bytesize },
    relationships: { assetLibrary: { data: { type: 'appAssetLibraries', id: APP } } } } })['data']
  img['attributes']['uploadOperations'].each do |op|
    uri = URI(op['url'])
    req = Net::HTTP.const_get(op['method'].capitalize).new(uri)
    (op['requestHeaders'] || []).each { |h| req[h['name']] = h['value'] }
    req.body = data[op['offset'], op['length']]
    res = Net::HTTP.start(uri.host, uri.port, use_ssl: true) { |h| h.request(req) }
    raise "part upload #{res.code}" unless res.code.to_i < 300
  end
  api(:patch, "/v1/appAssetLibraryImages/#{img['id']}", { data: { type: 'appAssetLibraryImages', id: img['id'], attributes: { uploaded: true } } })
  state = nil
  60.times do
    state = api(:get, "/v1/appAssetLibraryImages/#{img['id']}")['data']['attributes']['state']
    break unless %w[AWAITING_UPLOAD UPLOAD_COMPLETE].include?(state.to_s)
    sleep 5
  end
  puts "#{locale}: image #{img['id']} #{state}"
  existing = api(:get, "/v1/appStoreVersionLocalizations/#{loc['id']}/placements")['data']
  TYPES.each do |pt|
    existing.select { |p| p['attributes']['placementType'] == pt }.each { |p| api(:delete, "/v1/appAssetLibraryPlacements/#{p['id']}") }
    api(:post, '/v1/appAssetLibraryPlacements', { data: { type: 'appAssetLibraryPlacements',
      attributes: { placementType: pt, placementGroup: 'DEFAULT_PROFILE' },
      relationships: { appStoreVersionLocalization: { data: { type: 'appStoreVersionLocalizations', id: loc['id'] } },
                       image: { data: { type: 'appAssetLibraryImages', id: img['id'] } } } } })
  end
end

# Read back what is placed now.
locs.each do |l|
  ps = api(:get, "/v1/appStoreVersionLocalizations/#{l['id']}/placements")['data']
  creative = ps.select { |p| TYPES.include?(p['attributes']['placementType']) }
  shots = ps.count { |p| p['attributes']['placementType'] == 'APP_SCREENSHOT' }
  puts "#{l['attributes']['locale']}: #{shots} screenshots; " +
       creative.map { |p| "#{p['attributes']['placementType']} #{p['attributes']['state']}" }.join(', ')
end
