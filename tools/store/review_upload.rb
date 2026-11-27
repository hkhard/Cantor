# App Review attachment and notes for the version in review (called by
# tools/store/review_video.sh). Args: VIDEO NOTES_DOC [--submit]
require "json"
# Run as `ruby -e '... load ARGV[0]' review_upload.rb VIDEO DOC`, so ARGV[0]
# is this file itself.
args = ARGV.dup
args.shift if args.first && File.expand_path(args.first) == File.expand_path(__FILE__)
video, doc, submit = args
abort "usage: review_upload.rb VIDEO.mp4 NOTES.md [--submit]" unless video&.end_with?(".mp4") && doc&.end_with?(".md")
key = JSON.parse(File.read(File.expand_path("~/.appstoreconnect/choirbird-asc.json")))
Spaceship::ConnectAPI.token = Spaceship::ConnectAPI::Token.create(**key.transform_keys(&:to_sym).slice(:key_id, :issuer_id, :key, :in_house))

notes = File.read(doc, encoding: "UTF-8")[/\*\*Notes for the reviewer:\*\*.*?\n\n```\n(.*?)\n```/m, 1] or abort "no reviewer notes in #{doc}"
abort "notes are #{notes.size} characters (App Store Connect allows 4000)" if notes.size > 4000

app = Spaceship::ConnectAPI::App.find("se.nikielhard.choirbird")
v = app.get_edit_app_store_version || app.get_app_store_versions.first
puts "version #{v.version_string} (#{v.app_store_state})"
detail = v.fetch_app_store_review_detail

# Notes.
detail.update(attributes: { notes: notes })
puts "notes: #{notes.size} characters written"

# Attachment: replace any earlier recording.
(detail.app_store_review_attachments || []).each do |a|
  puts "removing old attachment #{a.file_name}"
  a.delete!
end
Spaceship::ConnectAPI::AppStoreReviewAttachment.create(app_store_review_detail_id: detail.id, path: video)
puts "attachment: #{File.basename(video)} uploaded"

# Read back.
d = v.fetch_app_store_review_detail
puts "read back: notes #{d.notes.to_s.size} characters, attachments #{(d.app_store_review_attachments || []).map(&:file_name).inspect}"

if submit == "--submit"
  sub = Spaceship::ConnectAPI.get_review_submissions(app_id: app.id, filter: { state: "UNRESOLVED_ISSUES,READY_FOR_REVIEW" }).first
  sub ||= Spaceship::ConnectAPI.post_review_submission(app_id: app.id, platform: "IOS").first
  begin
    Spaceship::ConnectAPI.post_review_submission_item(review_submission_id: sub.id, app_store_version_id: v.id)
  rescue StandardError => e
    puts "item: #{e.message[0, 120]} (already in the submission)"
  end
  Spaceship::ConnectAPI.patch_review_submission(review_submission_id: sub.id, attributes: { submitted: true })
  puts "resubmitted to App Review"
else
  puts "not submitted: reply in App Store Connect and press Resubmit to App Review (or rerun with --submit)"
end
