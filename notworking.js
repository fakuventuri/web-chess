function findAttackers(square, attackerColor) {
  const attackers = [];

  const moves = game.moves({ verbose: true });
  for (const move of moves) {
    if (move.to === square && move.color === attackerColor) {
      attackers.push(move.from);
    }
  }

  return moves;
}

function pathBetween(from, to) {
  const files = "abcdefgh";
  const fx = files.indexOf(from[0]);
  const fy = parseInt(from[1]);
  const tx = files.indexOf(to[0]);
  const ty = parseInt(to[1]);

  const dx = Math.sign(tx - fx);
  const dy = Math.sign(ty - fy);

  const path = [];
  let x = fx + dx;
  let y = fy + dy;
  while (x !== tx || y !== ty) {
    path.push(files[x] + y);
    x += dx;
    y += dy;
  }
  return path;
}
