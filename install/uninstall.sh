#!/usr/bin/env bash

set -e
#set -x

function printusage {
  echo "Usage: $0 <install_folder>"
  echo ""
  echo "Example:"
  echo "       $0 ~/rstream/"
}

if [[ $# -eq 1 && ( $1 == "-help" || $1 == "-h" ) ]];
then
  echo "$0 - uninstall the rtream server"
  echo "        - stop the services"
  echo "        - remove the service files"
  echo "        - remove the installation folder"
  echo ""
  printusage
  exit 0
fi

if [ $# -ne 1 ];
then
  echo "error: Incorrect number of arguments"
  printusage
  exit 1
fi

INSTALL_FOLDER=$1
SYSTEMD_FOLDER=~/.config/systemd/user/

systemctl --user stop rstream-caddy || true
rm -fr ${SYSTEMD_FOLDER}/rstream-caddy.service
echo "caddy service retired"
systemctl --user stop rstream-oauth2-proxy || true
rm -fr ${SYSTEMD_FOLDER}/rstream-oauth2-proxy.service
echo "oauth2_proxy service retired"
systemctl --user stop rstream-server || true
rm -fr ${SYSTEMD_FOLDER}/rstream-server.service
echo "rstream service retired"

systemctl --user stop rstream.target || true
rm -fr ${SYSTEMD_FOLDER}/rstream.target

rm -fr ${INSTALL_FOLDER}

systemctl --user daemon-reload

echo "uninstallation done"

