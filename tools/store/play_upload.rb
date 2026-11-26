#!/usr/bin/env ruby
# Upload ChoirBird to Google Play with the Android Publisher API (service
# account key ~/.android-keys/play-publisher.json, never in git).
#
#   tools/store/play_upload.rb status            read-only: tracks, listings
#   tools/store/play_upload.rb listing           texts + graphics (sv, en)
#   tools/store/play_upload.rb bundle TRACK      upload the release AAB to TRACK
#                                                (internal | alpha), as a draft
#   tools/store/play_upload.rb rollout TRACK     make TRACK's draft "completed"
#
# Texts come from docs/store/google-play.md, graphics from store-out/play
# (tools/store/make_play.sh). Runs with fastlane's gems:
#   D=$(dirname $(readlink -f $(which fastlane)))/../libexec
#   LC_ALL=en_US.UTF-8 GEM_HOME=$D GEM_PATH=$D /opt/homebrew/opt/ruby/bin/ruby tools/store/play_upload.rb status
require "google/apis/androidpublisher_v3"
require "googleauth"

ROOT = File.expand_path("../..", __dir__)
PKG = "se.nikielhard.choirbird"
AAB = File.join(ROOT, "app/src-tauri/gen/android/app/build/outputs/bundle/universalRelease/app-universal-release.aab")
PLAY = File.join(ROOT, "store-out/play")
LANGS = { "en-US" => ["English (en-GB)", "en"], "sv-SE" => ["Swedish (sv-SE)", "sv"] }
AP = Google::Apis::AndroidpublisherV3

def service
  s = AP::AndroidPublisherService.new
  s.authorization = Google::Auth::ServiceAccountCredentials.make_creds(
    json_key_io: File.open(File.expand_path("~/.android-keys/play-publisher.json")),
    scope: "https://www.googleapis.com/auth/androidpublisher"
  )
  s.client_options.send_timeout_sec = 1800
  s.client_options.read_timeout_sec = 1800
  s
end

# Fields of one language block in google-play.md.
def texts
  doc = File.read(File.join(ROOT, "docs/store/google-play.md"))
  out = {}
  LANGS.each do |code, (head, _)|
    block = doc.split(/^### /).find { |b| b.start_with?(head) } or abort "no #{head} in google-play.md"
    f = block.scan(/\*\*([^*]+)\*\*[^`]*?\n\n```\n(.*?)\n```/m).to_h { |k, v| [k.strip.downcase, v] }
    out[code] = f
  end
  out
end

def with_edit(svc)
  edit = svc.insert_edit(PKG)
  yield edit.id
  svc.commit_edit(PKG, edit.id)
  puts "committed edit #{edit.id}"
rescue StandardError
  svc.delete_edit(PKG, edit.id) rescue nil if edit
  raise
end

svc = service
case ARGV[0]
when "status"
  e = svc.insert_edit(PKG)
  (svc.list_edit_tracks(PKG, e.id).tracks || []).each do |t|
    puts "track #{t.track}: #{(t.releases || []).map { |r| "#{r.name} #{r.status} #{r.version_codes.inspect}" }.join('; ')}"
  end
  d = svc.get_edit_detail(PKG, e.id)
  puts "details: default=#{d.default_language} email=#{d.contact_email} web=#{d.contact_website}"
  (svc.list_edit_listings(PKG, e.id).listings || []).each { |l| puts "listing #{l.language}: #{l.title} / #{l.short_description}" }
  LANGS.each_key do |code|
    %w[phoneScreenshots sevenInchScreenshots tenInchScreenshots featureGraphic icon].each do |t|
      n = (svc.list_edit_images(PKG, e.id, code, t).images || []).size rescue 0
      puts "images #{code} #{t}: #{n}"
    end
  end
  svc.delete_edit(PKG, e.id)
when "listing"
  tx = texts
  with_edit(svc) do |id|
    svc.patch_edit_detail(PKG, id, AP::AppDetails.new(
      default_language: "en-US",
      contact_email: "choirbird-support@nikielhard.se",
      contact_website: "https://nikielhard.se/en/choirbird"
    ))
    LANGS.each do |code, (_, dir)|
      f = tx[code]
      svc.update_edit_listing(PKG, id, code, AP::Listing.new(
        language: code, title: f["app name"], short_description: f["short description"],
        full_description: f["full description"]
      ))
      puts "listing #{code}: text"
      {
        "icon" => ["icon-512.png"],
        "featureGraphic" => ["feature-#{dir}.png"],
        "phoneScreenshots" => Dir[File.join(PLAY, dir, "phone", "*.png")].sort,
        "sevenInchScreenshots" => Dir[File.join(PLAY, dir, "tablet-7", "*.png")].sort,
        "tenInchScreenshots" => Dir[File.join(PLAY, dir, "tablet-10", "*.png")].sort,
      }.each do |type, files|
        svc.deleteall_edit_image(PKG, id, code, type)
        files.each do |p|
          path = p.start_with?("/") ? p : File.join(PLAY, p)
          abort "missing #{path} (run tools/store/make_play.sh)" unless File.exist?(path)
          svc.upload_edit_image(PKG, id, code, type, upload_source: path, content_type: "image/png")
        end
        puts "listing #{code}: #{type} #{files.size}"
      end
    end
  end
when "bundle"
  track = ARGV[1] or abort "track?"
  abort "no AAB: #{AAB} (tools/android.sh release)" unless File.exist?(AAB)
  tx = texts
  with_edit(svc) do |id|
    b = svc.upload_edit_bundle(PKG, id, upload_source: AAB, content_type: "application/octet-stream", ack_bundle_installation_warning: true)
    puts "bundle version code #{b.version_code}"
    notes = LANGS.map { |code, _| AP::LocalizedText.new(language: code, text: tx[code]["release notes"]) }
    svc.update_edit_track(PKG, id, track, AP::Track.new(track: track, releases: [
      AP::TrackRelease.new(name: "1.0.0 (#{b.version_code})", status: "draft", version_codes: [b.version_code.to_s], release_notes: notes)
    ]))
    puts "track #{track}: draft release 1.0.0 (#{b.version_code})"
  end
when "rollout"
  track = ARGV[1] or abort "track?"
  with_edit(svc) do |id|
    t = svc.get_edit_track(PKG, id, track)
    r = t.releases.find { |x| x.status == "draft" } or abort "no draft on #{track}"
    r.status = "completed"
    svc.update_edit_track(PKG, id, track, AP::Track.new(track: track, releases: [r]))
    puts "track #{track}: #{r.name} completed"
  end
else
  abort "usage: play_upload.rb status|listing|bundle TRACK|rollout TRACK"
end
