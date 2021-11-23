#/usr/bin/env bash

APP_NAME=$1
if [ -z $APP_NAME ]
then
  echo "Please specify your app's name: $0 <app_name>"
  exit -1
fi


RTT_INFO=$(rust-nm -S target/thumbv7*/debug/$APP_NAME | grep RTT | sed -re 's/^(.*) (.*) D _SEGGER_RTT$/0x\1 0x\2/')

echo "monitor rtt setup $RTT_INFO \"SEGGER RTT\"" > .vscode/rtt.gdb
