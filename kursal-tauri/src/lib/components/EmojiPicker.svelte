<script lang="ts">
  import { onMount, tick } from "svelte";
  import { X, Search } from "lucide-svelte";

  let {
    onSelect,
    onClose,
    compact = false,
  }: {
    onSelect: (emoji: string) => void;
    onClose: () => void;
    compact?: boolean;
  } = $props();

  let searchQuery = $state("");
  let searchInput = $state<HTMLInputElement | null>(null);
  let activeCategory = $state("recent");
  let pickerEl = $state<HTMLDivElement | null>(null);

  const RECENT_KEY = "kursal_recent_emoji";
  const RECENT_COUNTS_KEY = "kursal_recent_emoji_counts";

  let recentCounts = $state<Record<string, number>>({});

  function adjustPosition() {
    if (!pickerEl) return;
    // Reset any prior shift before measuring
    pickerEl.style.setProperty("--x-shift", "0px");
    const rect = pickerEl.getBoundingClientRect();
    const vw = window.innerWidth;
    const margin = 8;
    let shift = 0;
    if (rect.right > vw - margin) shift = vw - margin - rect.right;
    if (rect.left + shift < margin) shift = margin - rect.left;
    pickerEl.style.setProperty("--x-shift", `${shift}px`);
  }

  onMount(() => {
    try {
      const storedCounts = localStorage.getItem(RECENT_COUNTS_KEY);
      if (storedCounts) {
        recentCounts = JSON.parse(storedCounts);
      } else {
        const legacy = localStorage.getItem(RECENT_KEY);
        if (legacy) {
          const arr: string[] = JSON.parse(legacy);
          const migrated: Record<string, number> = {};
          arr.forEach((e, i) => {
            migrated[e] = arr.length - i;
          });
          recentCounts = migrated;
          localStorage.setItem(RECENT_COUNTS_KEY, JSON.stringify(migrated));
        }
      }
    } catch {}
    tick().then(() => {
      searchInput?.focus();
      adjustPosition();
    });

    function handleClickOutside(e: MouseEvent) {
      if (pickerEl && !pickerEl.contains(e.target as Node)) {
        onClose();
      }
    }
    function handleEscape(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    function handleResize() { adjustPosition(); }
    document.addEventListener("mousedown", handleClickOutside);
    document.addEventListener("keydown", handleEscape);
    window.addEventListener("resize", handleResize);
    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
      document.removeEventListener("keydown", handleEscape);
      window.removeEventListener("resize", handleResize);
    };
  });

  function selectEmoji(emoji: string) {
    recentCounts = { ...recentCounts, [emoji]: (recentCounts[emoji] ?? 0) + 1 };
    try {
      localStorage.setItem(RECENT_COUNTS_KEY, JSON.stringify(recentCounts));
    } catch {}
    onSelect(emoji);
  }

  interface EmojiEntry { emoji: string; name: string; }
  interface Category { id: string; label: string; icon: string; emojis: EmojiEntry[]; }

  const categories: Category[] = [
    { id: "recent", label: "Recent", icon: "🕐", emojis: [] },
    { id: "smileys", label: "Smileys", icon: "😀", emojis: [
      { emoji: "😀", name: "grinning face" }, { emoji: "😃", name: "smiley" }, { emoji: "😄", name: "smile" },
      { emoji: "😁", name: "grin" }, { emoji: "😆", name: "laughing" }, { emoji: "😅", name: "sweat smile" },
      { emoji: "🤣", name: "rolling laughing" }, { emoji: "😂", name: "joy tears" }, { emoji: "🙂", name: "slightly smiling" },
      { emoji: "🙃", name: "upside down" }, { emoji: "😉", name: "wink" }, { emoji: "😊", name: "blush" },
      { emoji: "😇", name: "innocent halo" }, { emoji: "🥰", name: "hearts face" }, { emoji: "😍", name: "heart eyes" },
      { emoji: "🤩", name: "star struck" }, { emoji: "😘", name: "kissing heart" }, { emoji: "😗", name: "kissing" },
      { emoji: "😚", name: "kissing closed eyes" }, { emoji: "😙", name: "kissing smiling" }, { emoji: "🥲", name: "smiling tear" },
      { emoji: "😋", name: "yum delicious" }, { emoji: "😛", name: "tongue out" }, { emoji: "😜", name: "winking tongue" },
      { emoji: "🤪", name: "zany crazy" }, { emoji: "😝", name: "squinting tongue" }, { emoji: "🤑", name: "money mouth" },
      { emoji: "🤗", name: "hugging" }, { emoji: "🤭", name: "hand over mouth" }, { emoji: "🤫", name: "shushing" },
      { emoji: "🤔", name: "thinking" }, { emoji: "🫡", name: "salute" }, { emoji: "🤐", name: "zipper mouth" },
      { emoji: "🤨", name: "raised eyebrow" }, { emoji: "😐", name: "neutral" }, { emoji: "😑", name: "expressionless" },
      { emoji: "😶", name: "no mouth" }, { emoji: "🫥", name: "dotted line face" }, { emoji: "😏", name: "smirk" },
      { emoji: "😒", name: "unamused" }, { emoji: "🙄", name: "eye roll" }, { emoji: "😬", name: "grimacing" },
      { emoji: "🤥", name: "lying pinocchio" }, { emoji: "🫨", name: "shaking face" },
      { emoji: "😌", name: "relieved" }, { emoji: "😔", name: "pensive" }, { emoji: "😪", name: "sleepy" },
      { emoji: "🤤", name: "drooling" }, { emoji: "😴", name: "sleeping zzz" }, { emoji: "😷", name: "mask" },
      { emoji: "🤒", name: "thermometer sick" }, { emoji: "🤕", name: "bandage hurt" }, { emoji: "🤢", name: "nauseated" },
      { emoji: "🤮", name: "vomiting" }, { emoji: "🥵", name: "hot" }, { emoji: "🥶", name: "cold freezing" },
      { emoji: "🥴", name: "woozy drunk" }, { emoji: "😵", name: "dizzy" }, { emoji: "🤯", name: "exploding head mind blown" },
      { emoji: "🤠", name: "cowboy" }, { emoji: "🥳", name: "party" }, { emoji: "🥸", name: "disguised" },
      { emoji: "😎", name: "sunglasses cool" }, { emoji: "🤓", name: "nerd" }, { emoji: "🧐", name: "monocle" },
      { emoji: "😕", name: "confused" }, { emoji: "🫤", name: "diagonal mouth" }, { emoji: "😟", name: "worried" },
      { emoji: "🙁", name: "slightly frowning" }, { emoji: "😮", name: "open mouth" }, { emoji: "😯", name: "hushed" },
      { emoji: "😲", name: "astonished" }, { emoji: "😳", name: "flushed" }, { emoji: "🥺", name: "pleading" },
      { emoji: "🥹", name: "holding back tears" }, { emoji: "😦", name: "frowning open mouth" },
      { emoji: "😧", name: "anguished" }, { emoji: "😨", name: "fearful" }, { emoji: "😰", name: "anxious sweat" },
      { emoji: "😥", name: "sad relieved" }, { emoji: "😢", name: "cry" }, { emoji: "😭", name: "sobbing loudly crying" },
      { emoji: "😱", name: "screaming fear" }, { emoji: "😖", name: "confounded" }, { emoji: "😣", name: "persevering" },
      { emoji: "😞", name: "disappointed" }, { emoji: "😓", name: "downcast sweat" }, { emoji: "😩", name: "weary" },
      { emoji: "😫", name: "tired" }, { emoji: "🥱", name: "yawning" }, { emoji: "😤", name: "triumph steam nose" },
      { emoji: "😡", name: "pouting angry red" }, { emoji: "😠", name: "angry" }, { emoji: "🤬", name: "cursing swearing" },
      { emoji: "😈", name: "smiling devil horns" }, { emoji: "👿", name: "angry devil imp" }, { emoji: "💀", name: "skull dead" },
      { emoji: "☠️", name: "skull crossbones" }, { emoji: "💩", name: "poop" }, { emoji: "🤡", name: "clown" },
      { emoji: "👹", name: "ogre" }, { emoji: "👻", name: "ghost" }, { emoji: "👽", name: "alien" },
      { emoji: "🤖", name: "robot" }, { emoji: "😺", name: "smiling cat" }, { emoji: "😸", name: "grinning cat" },
      { emoji: "😹", name: "cat joy tears" }, { emoji: "😻", name: "heart eyes cat" }, { emoji: "🙈", name: "see no evil monkey" },
      { emoji: "🙉", name: "hear no evil monkey" }, { emoji: "🙊", name: "speak no evil monkey" },
    ]},
    { id: "gestures", label: "People", icon: "👋", emojis: [
      { emoji: "👋", name: "wave hello" }, { emoji: "🤚", name: "raised back hand" }, { emoji: "🖐️", name: "hand fingers splayed" },
      { emoji: "✋", name: "raised hand stop" }, { emoji: "🖖", name: "vulcan" }, { emoji: "🫱", name: "rightward hand" },
      { emoji: "🫲", name: "leftward hand" }, { emoji: "🫳", name: "palm down" }, { emoji: "🫴", name: "palm up" },
      { emoji: "🫷", name: "push left" }, { emoji: "🫸", name: "push right" },
      { emoji: "👌", name: "ok perfect" }, { emoji: "🤌", name: "pinched fingers italian" }, { emoji: "🤏", name: "pinching small" },
      { emoji: "✌️", name: "victory peace" }, { emoji: "🤞", name: "crossed fingers luck" }, { emoji: "🫰", name: "hand index thumb crossed" },
      { emoji: "🤟", name: "love you gesture" }, { emoji: "🤘", name: "rock on horns" }, { emoji: "🤙", name: "call me hand" },
      { emoji: "👈", name: "point left" }, { emoji: "👉", name: "point right" }, { emoji: "👆", name: "point up" },
      { emoji: "🖕", name: "middle finger" }, { emoji: "👇", name: "point down" }, { emoji: "☝️", name: "index up" },
      { emoji: "🫵", name: "point at viewer" }, { emoji: "👍", name: "thumbs up like" }, { emoji: "👎", name: "thumbs down dislike" },
      { emoji: "✊", name: "raised fist" }, { emoji: "👊", name: "fist bump punch" }, { emoji: "🤛", name: "left fist" },
      { emoji: "🤜", name: "right fist" }, { emoji: "👏", name: "clap" }, { emoji: "🙌", name: "raising hands celebrate" },
      { emoji: "🫶", name: "heart hands" }, { emoji: "👐", name: "open hands" }, { emoji: "🤲", name: "palms up together" },
      { emoji: "🤝", name: "handshake" }, { emoji: "🙏", name: "pray please thanks" }, { emoji: "✍️", name: "writing hand" },
      { emoji: "💅", name: "nail polish" }, { emoji: "🤳", name: "selfie" }, { emoji: "💪", name: "flexed biceps strong" },
      { emoji: "🦾", name: "mechanical arm" }, { emoji: "🦿", name: "mechanical leg" },
      { emoji: "👀", name: "eyes looking" }, { emoji: "👁️", name: "eye" }, { emoji: "👅", name: "tongue" },
      { emoji: "👄", name: "mouth lips" }, { emoji: "🫦", name: "biting lip" }, { emoji: "🧠", name: "brain" },
      { emoji: "👶", name: "baby" }, { emoji: "🧑", name: "person" }, { emoji: "👦", name: "boy" },
      { emoji: "👧", name: "girl" }, { emoji: "🧔", name: "beard" }, { emoji: "👨", name: "man" },
      { emoji: "👩", name: "woman" }, { emoji: "🧓", name: "older person" },
    ]},
    { id: "animals", label: "Nature", icon: "🐶", emojis: [
      { emoji: "🐶", name: "dog" }, { emoji: "🐱", name: "cat" }, { emoji: "🐭", name: "mouse" },
      { emoji: "🐹", name: "hamster" }, { emoji: "🐰", name: "rabbit bunny" }, { emoji: "🦊", name: "fox" },
      { emoji: "🐻", name: "bear" }, { emoji: "🐼", name: "panda" }, { emoji: "🐨", name: "koala" },
      { emoji: "🐯", name: "tiger" }, { emoji: "🦁", name: "lion" }, { emoji: "🐮", name: "cow" },
      { emoji: "🐷", name: "pig" }, { emoji: "🐸", name: "frog" }, { emoji: "🐵", name: "monkey" },
      { emoji: "🐔", name: "chicken" }, { emoji: "🐧", name: "penguin" }, { emoji: "🐦", name: "bird" },
      { emoji: "🦅", name: "eagle" }, { emoji: "🦉", name: "owl" }, { emoji: "🦇", name: "bat" },
      { emoji: "🐺", name: "wolf" }, { emoji: "🐗", name: "boar" }, { emoji: "🐴", name: "horse" },
      { emoji: "🦄", name: "unicorn" }, { emoji: "🐝", name: "bee honeybee" }, { emoji: "🪱", name: "worm" },
      { emoji: "🐛", name: "bug" }, { emoji: "🦋", name: "butterfly" }, { emoji: "🐌", name: "snail" },
      { emoji: "🐞", name: "ladybug" }, { emoji: "🐜", name: "ant" }, { emoji: "🪰", name: "fly" },
      { emoji: "🐢", name: "turtle tortoise" }, { emoji: "🐍", name: "snake" }, { emoji: "🦎", name: "lizard" },
      { emoji: "🐙", name: "octopus" }, { emoji: "🦑", name: "squid" }, { emoji: "🦀", name: "crab" },
      { emoji: "🦞", name: "lobster" }, { emoji: "🦐", name: "shrimp" }, { emoji: "🐠", name: "tropical fish" },
      { emoji: "🐟", name: "fish" }, { emoji: "🐬", name: "dolphin" }, { emoji: "🐳", name: "whale" },
      { emoji: "🦈", name: "shark" }, { emoji: "🐊", name: "crocodile" }, { emoji: "🐆", name: "leopard" },
      { emoji: "🦓", name: "zebra" }, { emoji: "🦍", name: "gorilla" }, { emoji: "🦧", name: "orangutan" },
      { emoji: "🐘", name: "elephant" }, { emoji: "🦛", name: "hippo" }, { emoji: "🦏", name: "rhino" },
      { emoji: "🐪", name: "camel" }, { emoji: "🦒", name: "giraffe" }, { emoji: "🐃", name: "water buffalo" },
      { emoji: "🌸", name: "cherry blossom" }, { emoji: "🌺", name: "hibiscus" }, { emoji: "🌻", name: "sunflower" },
      { emoji: "🌹", name: "rose" }, { emoji: "🌷", name: "tulip" }, { emoji: "🌼", name: "blossom" },
      { emoji: "🌿", name: "herb leaf" }, { emoji: "🍀", name: "four leaf clover luck" }, { emoji: "🍁", name: "maple leaf" },
      { emoji: "🍂", name: "fallen leaf autumn" }, { emoji: "🌴", name: "palm tree" }, { emoji: "🌵", name: "cactus" },
      { emoji: "🌲", name: "evergreen tree" }, { emoji: "🌳", name: "deciduous tree" }, { emoji: "🪴", name: "potted plant" },
      { emoji: "🌱", name: "seedling sprout" }, { emoji: "🍄", name: "mushroom" },
    ]},
    { id: "food", label: "Food", icon: "🍔", emojis: [
      { emoji: "🍎", name: "red apple" }, { emoji: "🍊", name: "orange tangerine" }, { emoji: "🍋", name: "lemon" },
      { emoji: "🍌", name: "banana" }, { emoji: "🍉", name: "watermelon" }, { emoji: "🍇", name: "grapes" },
      { emoji: "🍓", name: "strawberry" }, { emoji: "🫐", name: "blueberries" }, { emoji: "🍈", name: "melon" },
      { emoji: "🍒", name: "cherries" }, { emoji: "🍑", name: "peach" }, { emoji: "🥭", name: "mango" },
      { emoji: "🍍", name: "pineapple" }, { emoji: "🥥", name: "coconut" }, { emoji: "🥝", name: "kiwi" },
      { emoji: "🍅", name: "tomato" }, { emoji: "🥑", name: "avocado" }, { emoji: "🍆", name: "eggplant" },
      { emoji: "🥔", name: "potato" }, { emoji: "🥕", name: "carrot" }, { emoji: "🌽", name: "corn" },
      { emoji: "🌶️", name: "hot pepper chili" }, { emoji: "🥒", name: "cucumber" }, { emoji: "🥦", name: "broccoli" },
      { emoji: "🧄", name: "garlic" }, { emoji: "🧅", name: "onion" },
      { emoji: "🍞", name: "bread" }, { emoji: "🥐", name: "croissant" }, { emoji: "🥖", name: "baguette" },
      { emoji: "🥨", name: "pretzel" }, { emoji: "🧀", name: "cheese" }, { emoji: "🥚", name: "egg" },
      { emoji: "🍳", name: "cooking fried egg" }, { emoji: "🥓", name: "bacon" }, { emoji: "🥩", name: "steak meat" },
      { emoji: "🍗", name: "chicken leg" }, { emoji: "🍖", name: "meat bone" }, { emoji: "🌭", name: "hot dog" },
      { emoji: "🍔", name: "hamburger burger" }, { emoji: "🍟", name: "french fries" }, { emoji: "🍕", name: "pizza" },
      { emoji: "🥪", name: "sandwich" }, { emoji: "🌮", name: "taco" }, { emoji: "🌯", name: "burrito" },
      { emoji: "🥗", name: "salad" }, { emoji: "🍝", name: "spaghetti pasta" }, { emoji: "🍜", name: "ramen noodles" },
      { emoji: "🍲", name: "pot food stew" }, { emoji: "🍛", name: "curry rice" }, { emoji: "🍣", name: "sushi" },
      { emoji: "🍱", name: "bento box" }, { emoji: "🥟", name: "dumpling" }, { emoji: "🍩", name: "donut doughnut" },
      { emoji: "🍪", name: "cookie" }, { emoji: "🎂", name: "birthday cake" }, { emoji: "🍰", name: "cake slice" },
      { emoji: "🧁", name: "cupcake" }, { emoji: "🍫", name: "chocolate bar" }, { emoji: "🍬", name: "candy" },
      { emoji: "🍭", name: "lollipop" }, { emoji: "🍮", name: "custard flan" }, { emoji: "🍯", name: "honey" },
      { emoji: "☕", name: "coffee hot" }, { emoji: "🍵", name: "tea" }, { emoji: "🧃", name: "juice box" },
      { emoji: "🥤", name: "cup straw" }, { emoji: "🍺", name: "beer" }, { emoji: "🍻", name: "beers cheers" },
      { emoji: "🥂", name: "champagne clinking glasses" }, { emoji: "🍷", name: "wine" }, { emoji: "🍸", name: "cocktail martini" },
      { emoji: "🧊", name: "ice cube" },
    ]},
    { id: "activities", label: "Activities", icon: "⚽", emojis: [
      { emoji: "⚽", name: "soccer football" }, { emoji: "🏀", name: "basketball" }, { emoji: "🏈", name: "american football" },
      { emoji: "⚾", name: "baseball" }, { emoji: "🥎", name: "softball" }, { emoji: "🎾", name: "tennis" },
      { emoji: "🏐", name: "volleyball" }, { emoji: "🏉", name: "rugby" }, { emoji: "🥏", name: "frisbee disc" },
      { emoji: "🎱", name: "pool billiards 8ball" }, { emoji: "🏓", name: "ping pong table tennis" },
      { emoji: "🏸", name: "badminton" }, { emoji: "🥊", name: "boxing glove" }, { emoji: "🥋", name: "martial arts" },
      { emoji: "🏅", name: "medal" }, { emoji: "🥇", name: "gold medal first" }, { emoji: "🥈", name: "silver medal second" },
      { emoji: "🥉", name: "bronze medal third" }, { emoji: "🏆", name: "trophy winner" }, { emoji: "🎮", name: "video game controller" },
      { emoji: "🕹️", name: "joystick" }, { emoji: "🎲", name: "dice game" }, { emoji: "🧩", name: "puzzle piece" },
      { emoji: "♟️", name: "chess pawn" }, { emoji: "🎯", name: "direct hit bullseye target" }, { emoji: "🎳", name: "bowling" },
      { emoji: "🎪", name: "circus tent" }, { emoji: "🎭", name: "performing arts theater" }, { emoji: "🎨", name: "art palette" },
      { emoji: "🎬", name: "clapperboard movie" }, { emoji: "🎤", name: "microphone karaoke" }, { emoji: "🎧", name: "headphones" },
      { emoji: "🎼", name: "musical score" }, { emoji: "🎹", name: "piano keyboard" }, { emoji: "🎷", name: "saxophone" },
      { emoji: "🎸", name: "guitar" }, { emoji: "🎺", name: "trumpet" }, { emoji: "🥁", name: "drum" },
      { emoji: "🎻", name: "violin" }, { emoji: "🪗", name: "accordion" }, { emoji: "🎵", name: "musical note" },
      { emoji: "🎶", name: "musical notes" },
    ]},
    { id: "travel", label: "Travel", icon: "✈️", emojis: [
      { emoji: "🚗", name: "car automobile" }, { emoji: "🚕", name: "taxi cab" }, { emoji: "🚌", name: "bus" },
      { emoji: "🚎", name: "trolleybus" }, { emoji: "🏎️", name: "racing car" }, { emoji: "🚓", name: "police car" },
      { emoji: "🚑", name: "ambulance" }, { emoji: "🚒", name: "fire engine truck" }, { emoji: "🚐", name: "minibus van" },
      { emoji: "🛻", name: "pickup truck" }, { emoji: "🚚", name: "delivery truck" }, { emoji: "🚂", name: "locomotive train" },
      { emoji: "🚆", name: "train" }, { emoji: "🚇", name: "metro subway" }, { emoji: "✈️", name: "airplane" },
      { emoji: "🚀", name: "rocket launch" }, { emoji: "🛸", name: "flying saucer ufo" }, { emoji: "🚁", name: "helicopter" },
      { emoji: "⛵", name: "sailboat" }, { emoji: "🚤", name: "speedboat" }, { emoji: "🛥️", name: "motor boat" },
      { emoji: "🛳️", name: "cruise ship" }, { emoji: "🚲", name: "bicycle bike" }, { emoji: "🛴", name: "scooter kick" },
      { emoji: "🏍️", name: "motorcycle" },
      { emoji: "🏠", name: "house home" }, { emoji: "🏡", name: "house garden" }, { emoji: "🏢", name: "office building" },
      { emoji: "🏰", name: "castle" }, { emoji: "🏯", name: "japanese castle" }, { emoji: "🗼", name: "tokyo tower" },
      { emoji: "🗽", name: "statue liberty" }, { emoji: "⛪", name: "church" }, { emoji: "🕌", name: "mosque" },
      { emoji: "🏔️", name: "snow mountain" }, { emoji: "⛰️", name: "mountain" }, { emoji: "🌋", name: "volcano" },
      { emoji: "🏝️", name: "island desert" }, { emoji: "🏖️", name: "beach umbrella" },
      { emoji: "🌅", name: "sunrise" }, { emoji: "🌄", name: "sunrise mountains" }, { emoji: "🌇", name: "sunset" },
      { emoji: "🌃", name: "night stars" }, { emoji: "🌉", name: "bridge night" }, { emoji: "🌌", name: "milky way galaxy" },
      { emoji: "🎆", name: "fireworks" }, { emoji: "🎇", name: "sparkler" },
    ]},
    { id: "objects", label: "Objects", icon: "💡", emojis: [
      { emoji: "⌚", name: "watch" }, { emoji: "📱", name: "phone mobile" }, { emoji: "💻", name: "laptop computer" },
      { emoji: "⌨️", name: "keyboard" }, { emoji: "🖥️", name: "desktop computer monitor" }, { emoji: "🖨️", name: "printer" },
      { emoji: "🖱️", name: "mouse computer" }, { emoji: "💾", name: "floppy disk save" }, { emoji: "💿", name: "cd optical disk" },
      { emoji: "📀", name: "dvd" }, { emoji: "📷", name: "camera" }, { emoji: "📹", name: "video camera" },
      { emoji: "🔍", name: "magnifying glass search" }, { emoji: "🔬", name: "microscope" }, { emoji: "🔭", name: "telescope" },
      { emoji: "💡", name: "light bulb idea" }, { emoji: "🔦", name: "flashlight" }, { emoji: "🔋", name: "battery" },
      { emoji: "🔌", name: "plug electric" }, { emoji: "📡", name: "satellite antenna" },
      { emoji: "💰", name: "money bag" }, { emoji: "💳", name: "credit card" }, { emoji: "💎", name: "gem diamond" },
      { emoji: "🔑", name: "key" }, { emoji: "🗝️", name: "old key" }, { emoji: "🔒", name: "lock locked" },
      { emoji: "🔓", name: "unlock unlocked" }, { emoji: "🛡️", name: "shield" },
      { emoji: "🔧", name: "wrench tool" }, { emoji: "🔨", name: "hammer" }, { emoji: "⚙️", name: "gear settings" },
      { emoji: "🧲", name: "magnet" }, { emoji: "⚡", name: "lightning bolt zap" },
      { emoji: "🔥", name: "fire hot flame" }, { emoji: "💧", name: "water droplet" }, { emoji: "🌊", name: "wave ocean" },
      { emoji: "📦", name: "package box" }, { emoji: "📫", name: "mailbox" }, { emoji: "📝", name: "memo note" },
      { emoji: "📎", name: "paperclip" }, { emoji: "📌", name: "pin pushpin" }, { emoji: "📐", name: "triangular ruler" },
      { emoji: "✂️", name: "scissors cut" }, { emoji: "📚", name: "books" }, { emoji: "📖", name: "open book" },
      { emoji: "🔗", name: "link chain" }, { emoji: "💊", name: "pill medicine" }, { emoji: "🩹", name: "bandage adhesive" },
      { emoji: "🧬", name: "dna" }, { emoji: "🧪", name: "test tube" }, { emoji: "🧫", name: "petri dish" },
    ]},
    { id: "symbols", label: "Symbols", icon: "❤️", emojis: [
      { emoji: "❤️", name: "red heart love" }, { emoji: "🧡", name: "orange heart" }, { emoji: "💛", name: "yellow heart" },
      { emoji: "💚", name: "green heart" }, { emoji: "💙", name: "blue heart" }, { emoji: "💜", name: "purple heart" },
      { emoji: "🖤", name: "black heart" }, { emoji: "🤍", name: "white heart" }, { emoji: "🤎", name: "brown heart" },
      { emoji: "💔", name: "broken heart" }, { emoji: "❤️‍🔥", name: "heart fire" }, { emoji: "💕", name: "two hearts" },
      { emoji: "💞", name: "revolving hearts" }, { emoji: "💓", name: "beating heart" }, { emoji: "💗", name: "growing heart" },
      { emoji: "💖", name: "sparkling heart" }, { emoji: "💘", name: "heart arrow cupid" }, { emoji: "💝", name: "heart ribbon gift" },
      { emoji: "💟", name: "heart decoration" },
      { emoji: "✅", name: "check mark done" }, { emoji: "❌", name: "cross mark wrong" }, { emoji: "❓", name: "question mark" },
      { emoji: "❗", name: "exclamation" }, { emoji: "‼️", name: "double exclamation" }, { emoji: "⁉️", name: "exclamation question" },
      { emoji: "💯", name: "hundred perfect" }, { emoji: "🔴", name: "red circle" }, { emoji: "🟢", name: "green circle" },
      { emoji: "🔵", name: "blue circle" }, { emoji: "🟡", name: "yellow circle" }, { emoji: "🟣", name: "purple circle" },
      { emoji: "⚪", name: "white circle" }, { emoji: "⚫", name: "black circle" },
      { emoji: "🚫", name: "prohibited no" }, { emoji: "⛔", name: "no entry" }, { emoji: "📛", name: "name badge" },
      { emoji: "⭐", name: "star" }, { emoji: "🌟", name: "glowing star" }, { emoji: "✨", name: "sparkles" },
      { emoji: "💫", name: "dizzy star" }, { emoji: "🌈", name: "rainbow" }, { emoji: "☀️", name: "sun" },
      { emoji: "🌙", name: "crescent moon" }, { emoji: "⭕", name: "circle" }, { emoji: "♻️", name: "recycle" },
      { emoji: "🏳️", name: "white flag" }, { emoji: "🏴", name: "black flag" }, { emoji: "🚩", name: "red flag" },
      { emoji: "🎉", name: "party popper tada" }, { emoji: "🎊", name: "confetti ball" }, { emoji: "🎁", name: "gift present wrapped" },
      { emoji: "🎀", name: "ribbon bow" }, { emoji: "🏷️", name: "label tag" },
      { emoji: "💤", name: "sleep zzz" }, { emoji: "💬", name: "speech bubble chat" }, { emoji: "💭", name: "thought bubble" },
      { emoji: "🗯️", name: "anger bubble" }, { emoji: "👁️‍🗨️", name: "eye speech bubble" },
    ]},
  ];

  const filteredEmojis = $derived.by(() => {
    const q = searchQuery.toLowerCase().trim();
    if (!q) return null; // show categories view
    const results: EmojiEntry[] = [];
    for (const cat of categories) {
      if (cat.id === "recent") continue;
      for (const e of cat.emojis) {
        if (e.name.includes(q) || e.emoji === q) {
          results.push(e);
        }
      }
    }
    return results;
  });

  const recentCategory = $derived.by(() => {
    const limit = compact ? 7 : 8;
    return Object.entries(recentCounts)
      .sort((a, b) => b[1] - a[1])
      .slice(0, limit)
      .map(([emoji]) => ({ emoji, name: "recent" }));
  });

  function getCategoryEmojis(cat: Category): EmojiEntry[] {
    if (cat.id === "recent") return recentCategory;
    return cat.emojis;
  }
</script>

<div class="emoji-picker" class:compact bind:this={pickerEl}>
  <div class="picker-header">
    <div class="search-box">
      <Search size={14} />
      <input
        type="text"
        placeholder="Search emoji..."
        bind:value={searchQuery}
        bind:this={searchInput}
      />
      {#if searchQuery}
        <button class="clear-search" onclick={() => { searchQuery = ""; searchInput?.focus(); }}>
          <X size={12} />
        </button>
      {/if}
    </div>
  </div>

  {#if !filteredEmojis}
    <div class="category-tabs">
      {#each categories as cat (cat.id)}
        <button
          class="cat-tab"
          class:active={activeCategory === cat.id}
          onclick={() => {
            activeCategory = cat.id;
            const section = pickerEl?.querySelector(`[data-category="${cat.id}"]`);
            section?.scrollIntoView({ behavior: "smooth", block: "start" });
          }}
          title={cat.label}
        >{cat.icon}</button>
      {/each}
    </div>
  {/if}

  <div class="emoji-grid-container">
    {#if filteredEmojis}
      {#if filteredEmojis.length === 0}
        <div class="no-results">No emoji found</div>
      {:else}
        <div class="emoji-grid">
          {#each filteredEmojis as entry (entry.emoji + entry.name)}
            <button
              class="emoji-btn"
              title={entry.name}
              onclick={() => selectEmoji(entry.emoji)}
            >{entry.emoji}</button>
          {/each}
        </div>
      {/if}
    {:else}
      {#each categories as cat (cat.id)}
        {@const emojis = getCategoryEmojis(cat)}
        {#if emojis.length > 0}
          <div class="category-section" data-category={cat.id}>
            <div class="category-label">{cat.label}</div>
            <div class="emoji-grid">
              {#each emojis as entry (entry.emoji)}
                <button
                  class="emoji-btn"
                  title={entry.name}
                  onclick={() => selectEmoji(entry.emoji)}
                >{entry.emoji}</button>
              {/each}
            </div>
          </div>
        {/if}
      {/each}
    {/if}
  </div>
</div>

<style>
  .emoji-picker {
    width: min(352px, calc(100vw - 16px));
    max-height: min(420px, calc(100vh - 80px));
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: 14px;
    box-shadow: var(--glow);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    z-index: 200;
    transform: translateX(var(--x-shift, 0));
  }
  .emoji-picker.compact {
    width: min(288px, calc(100vw - 16px));
    max-height: min(320px, calc(100vh - 40px));
    border-radius: 12px;
  }
  .emoji-picker.compact .picker-header {
    padding: 8px 8px 0;
  }
  .emoji-picker.compact .search-box {
    padding: 6px 8px;
    border-radius: 8px;
  }
  .emoji-picker.compact .search-box input {
    font-size: 12px;
  }
  .emoji-picker.compact .category-tabs {
    padding: 6px 8px 4px;
  }
  .emoji-picker.compact .cat-tab {
    padding: 4px 0;
    font-size: 14px;
  }
  .emoji-picker.compact .emoji-grid-container {
    padding: 0 8px 8px;
  }
  .emoji-picker.compact .emoji-grid {
    grid-template-columns: repeat(7, 1fr);
  }
  .emoji-picker.compact .emoji-btn {
    font-size: 18px;
    border-radius: 6px;
  }
  .emoji-picker.compact .category-label {
    font-size: 10px;
    padding: 3px 2px 2px;
  }

  .picker-header {
    padding: 10px 10px 0;
  }

  .search-box {
    display: flex;
    align-items: center;
    gap: 8px;
    background: var(--surface-soft);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 8px 10px;
    color: var(--text-muted);
  }

  .search-box input {
    flex: 1;
    background: transparent;
    border: none;
    outline: none;
    color: var(--text-primary);
    font-size: 13px;
  }

  .search-box input::placeholder {
    color: var(--text-muted);
  }

  .clear-search {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: var(--bg-hover);
    color: var(--text-muted);
    cursor: pointer;
    transition: background 0.15s;
  }

  .clear-search:hover {
    background: color-mix(in srgb, var(--bg-hover) 70%, var(--text-muted));
  }

  .category-tabs {
    display: flex;
    gap: 2px;
    padding: 8px 10px 6px;
    border-bottom: 1px solid var(--border);
  }

  .cat-tab {
    flex: 1;
    padding: 6px 0;
    font-size: 16px;
    border-radius: 8px;
    cursor: pointer;
    transition: background 0.15s;
    text-align: center;
    line-height: 1;
  }

  .cat-tab:hover {
    background: var(--bg-hover);
  }

  .cat-tab.active {
    background: var(--accent-dim);
  }

  .emoji-grid-container {
    flex: 1;
    overflow-y: auto;
    padding: 0 10px 10px;
  }

  .category-section {
    margin-bottom: 8px;
  }

  .category-label {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-muted);
    padding: 4px 4px 3px;
    position: sticky;
    top: 0;
    background: var(--bg-secondary);
    z-index: 1;
  }

  .emoji-grid {
    display: grid;
    grid-template-columns: repeat(8, 1fr);
    gap: 2px;
  }

  .emoji-btn {
    width: 100%;
    aspect-ratio: 1;
    font-size: 22px;
    border-radius: 8px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background 0.12s, transform 0.12s;
    line-height: 1;
  }

  .emoji-btn:hover {
    background: var(--bg-hover);
    transform: scale(1.15);
  }

  .no-results {
    text-align: center;
    color: var(--text-muted);
    font-size: 13px;
    padding: 32px 0;
  }

  @media (max-width: 500px) {
    .emoji-picker {
      max-height: min(360px, calc(100vh - 60px));
    }
  }
</style>
