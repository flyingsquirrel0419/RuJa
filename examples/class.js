// Class hierarchy with super
class Animal {
    constructor(name) { this.name = name; }
    speak() { return this.name + " makes a sound"; }
}

class Dog extends Animal {
    speak() { return this.name + " barks"; }
}

let d = new Dog("Rex");
console.log(d.speak()); // Rex barks
