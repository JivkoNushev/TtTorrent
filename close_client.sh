ps aux | grep "/tttorrent-client" | grep -v grep | awk '{print $2}' | xargs kill -9