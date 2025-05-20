if test -z $OSU_API_CLIENT_ID || test -z $OSU_API_CLIENT_SECRET
  echo "You need to set OSU_API_CLIENT_ID and OSU_API_CLIENT_SECRET"
  exit 1
end

if set -q OSU_API_TOKEN
  echo "OSU_API_TOKEN already set. Unset it to re-request the token"
  echo "set --erase OSU_API_TOKEN"
  exit 1
end

echo "Requesting token from osu! API..."

set TOKEN_JSON (curl --request POST \
    "https://osu.ppy.sh/oauth/token" \
    --header "Accept: application/json" \
    --header "Content-Type: application/x-www-form-urlencoded" \
    --fail-with-body \
    --silent \
    --data "client_id=$OSU_API_CLIENT_ID&client_secret=$OSU_API_CLIENT_SECRET&grant_type=client_credentials&scope=public")

if test $status -ne 0
  echo "Error while trying to request osu! API token:"
  echo $TOKEN_JSON
  exit 1
end

set OSU_API_TOKEN (echo $TOKEN_JSON | jq -r ".access_token")
if test $status -ne 0
  echo "Error while trying to request osu! API token:"
  echo $TOKEN_JSON
  exit 1
end

alias curl-osu='curl --header "Authorization: Bearer $OSU_API_TOKEN" \
         --header "Content-Type: application/json" \
         --header "Accept: application/json"'

echo "OSU_API_TOKEN set. Use `curl-osu` for a `curl` wrapper with the token already loaded."
