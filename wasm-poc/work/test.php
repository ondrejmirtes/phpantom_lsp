<?php

class Greeter
{
    public function hello(string $name): string
    {
        return 'Hello ' . $name;
    }
}

$g = new Greeter();
echo $g->hello();    // missing required argument $name
echo $g->goodbye();  // method does not exist on Greeter
