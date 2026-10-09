const d0 = 0;
function C(){}
C.prototype.val = function(){ return new d0(); };
console.log(new C().val(), new C().val());
