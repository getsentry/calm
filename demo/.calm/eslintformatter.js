function convertSeverity(level) {
  switch (level) {
    case 2: { return 'error'; }
    case 1: { return 'warning'; }
    default: { return 'info'; }
  };
}

module.exports = function(results) {
  for (var match of results) {
    for (var msg of match.messages) {
      if (!msg.fatal && msg.message.match(/--no-ignore/)) {
        continue;
      }
      console.log(JSON.stringify({
        filename: match.filePath,
        code: msg.ruleId || 'bad-syntax',
        level: convertSeverity(msg.severity),
        message: msg.message,
        line: msg.line,
        column: msg.column,
      }));
    }
  }
}
