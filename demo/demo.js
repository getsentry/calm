var foo = 42;

function badFunction() {
  throw new Error('Test');
}

function goodFunction() {
  badderFunction();
}
