#!/usr/bin/env bash

set -e
#set -x

function printusage {
  echo "Usage: $0 <installation_config> <install_folder>"
  echo ""
  echo "Example:"
  echo "       $0 config ~/rstream/"
  echo ""
  echo "Install config:"
  echo "  The installation config must contain the following variables:"
  echo "       OAUTH2_PROXY_URL=https://github.com/oauth2-proxy/oauth2-proxy/releases/download/v7.6.0/oauth2-proxy-v7.6.0.linux-amd64.tar.gz"
  echo "       CADDY_URL=https://github.com/caddyserver/caddy/releases/download/v2.8.4/caddy_2.8.4_linux_amd64.tar.gz"
  echo "       RSTREAM_URL=https://github.com/jdmichaud/rstream/releases/download/0.1.0/rstream-x86_64-linux.tgz"
  echo "       GOOGLE_CLIENT_ID=..."
  echo "       GOOGLE_CLIENT_SECRET=..."
  echo "       DOMAIN=... (ex: gmail.com)"
  echo ""
}

if [[ $# -eq 1 && ( $1 == "-help" || $1 == "-h" ) ]];
then
  echo "$0 - install a rtream server"
  echo "     composed of:"
  echo "        - a reverse proxy (caddy)"
  echo "        - a oauth module (oauth2_proxy)"
  echo "        - the rstream binary"
  echo ""
  printusage
  exit 0
fi

if [ $# -ne 2 ];
then
  echo "error: Incorrect number of arguments"
  printusage
  exit 1
fi

CONFIG_FILE=$1
INSTALL_FOLDER=$2
SYSTEMD_FOLDER=~/.config/systemd/user/
DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}/rstream"

if [ ! -f ${CONFIG_FILE} ]; then
  echo "error: config file ${CONFIG_FILE} not found!"
  echo ""
  printusage
  exit 1
fi

# https://stackoverflow.com/a/30969768/2603925
set -o allexport && source ${CONFIG_FILE} && set +o allexport

download_executable() {
  executable=$1
  url=$2
  exepath=$3
  install_folder=$4

  # Check the presence of the unit
  if ! systemctl --user is-active --quiet ${executable}
  then
    # Unit is not present, download
    if ! command -v ${executable} &> /dev/null
    then
      # Check the presence of the tarball
      if [ ! -f ${install_folder}/${executable}.tgz ]
      then
        # Download tarball
        echo "Downloading $url ..."
        curl -sL $url -o ${install_folder}/${executable}.tgz
      else
        echo "$executable already present"
      fi
      # Unzip and copy executable
      mkdir -p /tmp/${executable}-install
      tar zxf ${install_folder}/${executable}.tgz -C /tmp/${executable}-install
      cp /tmp/${executable}-install/${exepath} ${install_folder}
    fi
  fi
}

install_service() {
  unit_file=$1
  service_name="${unit_file%.*}"
  # Create and install unit
  sed -e "s%@INSTALL_FOLDER@%${INSTALL_FOLDER}/%g" $unit_file.in > ${SYSTEMD_FOLDER}/$unit_file
  sed -i -e "s%@DATA_HOME@%${DATA_HOME}/%g" ${SYSTEMD_FOLDER}/$unit_file
  chmod 600 ${SYSTEMD_FOLDER}/$unit_file
  echo "starting $service_name"
  systemctl --user daemon-reload
}

mkdir -p ${INSTALL_FOLDER}
mkdir -p ${SYSTEMD_FOLDER}

# Create the config file
mkdir -p ${DATA_HOME}

# Install the rstream target
cp rstream.target.in ${SYSTEMD_FOLDER}/rstream.target
systemctl --user daemon-reload

# Install the caddy reverse proxy service
download_executable caddy ${CADDY_URL} caddy ${INSTALL_FOLDER}
install_service rstream-caddy.service

# Install the oauth2-proxy service
#  Copy the email list
cp authenticated-emails-file ${INSTALL_FOLDER}/
#  Get the file name from he url
extract_folder=$(basename ${OAUTH2_PROXY_URL})
extract_folder=${extract_folder%.*}
extract_folder=${extract_folder%.*}
download_executable oauth2-proxy ${OAUTH2_PROXY_URL} ${extract_folder}/oauth2-proxy ${INSTALL_FOLDER}
#  Interpolate the variables in the unit file
sed -e "s%@INSTALL_FOLDER@%${INSTALL_FOLDER}/%g" rstream-oauth2-proxy.service.in > ${SYSTEMD_FOLDER}/rstream-oauth2-proxy.service
sed -i -e "s%@DATA_HOME@%${DATA_HOME}/%g" ${SYSTEMD_FOLDER}/rstream-oauth2-proxy.service
COOKIE_SECRET=$(dd if=/dev/urandom bs=32 count=1 2>/dev/null | base64 | tr -d -- '\n' | tr -- '+/' '-_')
sed -i -e "s%@COOKIE_SECRET@%${COOKIE_SECRET}%g" ${SYSTEMD_FOLDER}/rstream-oauth2-proxy.service
sed -i -e "s%@GOOGLE_CLIENT_ID@%${GOOGLE_CLIENT_ID}%g" ${SYSTEMD_FOLDER}/rstream-oauth2-proxy.service
sed -i -e "s%@GOOGLE_CLIENT_SECRET@%${GOOGLE_CLIENT_SECRET}%g" ${SYSTEMD_FOLDER}/rstream-oauth2-proxy.service
sed -i -e "s%@DOMAIN@%${DOMAIN}%g" ${SYSTEMD_FOLDER}/rstream-oauth2-proxy.service
chmod 600 ${SYSTEMD_FOLDER}/rstream-oauth2-proxy.service
echo "starting rstream-oauth2-proxy"

# Install rstream
download_executable rstream ${RSTREAM_URL} target/x86_64-unknown-linux-musl/release-with-debug/rstream-x86_64-linux ${INSTALL_FOLDER}
install_service rstream-server.service

# Enable the target
systemctl --user enable rstream.target
systemctl --user start rstream.target
echo "rstream target started"

echo "installation done"

echo ""
echo "Make sure that your project is properly configured in the Google Cloud Console"
echo 'In "API & services" > Credentials" create your credentials and make sure the you'
echo 'have configured the "Authorized redirect URIs" to https://localhost:8000/oauth2/callback'
echo ""
echo 'Make sure you have a file named "authenticated-emails-file" with the list of gmail address'
echo "authorized on your application."
